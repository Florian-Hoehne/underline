//! PlantUML through its official build compiled to JavaScript, run in an
//! embedded QuickJS (ADR-009, spike result): no Java, no network.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use rquickjs::{Context, Ctx, Function, Runtime};

use super::{DiagramError, DiagramRenderer};

/// The engine, `@plantuml/core` 1.2026.9 (vendor/plantuml/README.md).
const ENGINE: &str = include_str!("../../vendor/plantuml/plantuml.js");
const THEMES: &str = include_str!("../../vendor/plantuml/themes.js");
const HOST: &str = include_str!("host.js");
/// How the engine module exports its functions, and what the host makes of it.
const EXPORT: &str = "export{C as render,D as renderToString};";
const EXPORT_AS_GLOBAL: &str = "globalThis.__plantuml={render:C,renderToString:D};";

/// A render that takes longer is stopped (ADR-009).
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// The PlantUML renderer; the engine is started on the first diagram.
pub struct PlantUml {
    engine: Option<Engine>,
    timeout: Duration,
}

struct Engine {
    /// Kept alive with the context.
    _runtime: Runtime,
    context: Context,
    deadline: Arc<Mutex<Option<Instant>>>,
}

impl Default for PlantUml {
    fn default() -> Self {
        Self::new()
    }
}

impl PlantUml {
    pub fn new() -> Self {
        Self {
            engine: None,
            timeout: TIMEOUT,
        }
    }

    /// The same renderer with another time limit.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            engine: None,
            timeout,
        }
    }

    fn engine(&mut self) -> Result<&Engine, DiagramError> {
        if self.engine.is_none() {
            self.engine = Some(Engine::start()?);
        }
        Ok(self.engine.as_ref().expect("started"))
    }
}

impl Engine {
    fn start() -> Result<Self, DiagramError> {
        let failed = |e: rquickjs::Error| DiagramError::Engine(e.to_string());
        let runtime = Runtime::new().map_err(failed)?;
        runtime.set_memory_limit(1 << 30);
        runtime.set_max_stack_size(16 << 20);
        let deadline: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let watched = Arc::clone(&deadline);
        runtime.set_interrupt_handler(Some(Box::new(move || {
            watched
                .lock()
                .ok()
                .and_then(|d| *d)
                .is_some_and(|d| Instant::now() > d)
        })));
        let context = Context::full(&runtime).map_err(failed)?;
        let engine = ENGINE.replacen(EXPORT, EXPORT_AS_GLOBAL, 1);
        context.with(|ctx: Ctx| -> Result<(), DiagramError> {
            let globals = ctx.globals();
            let log = Function::new(ctx.clone(), |_message: String| {}).map_err(failed)?;
            globals.set("__log", log).map_err(failed)?;
            let measure = Function::new(ctx.clone(), measure).map_err(failed)?;
            globals.set("__measure", measure).map_err(failed)?;
            for script in [HOST, THEMES, engine.as_str()] {
                ctx.eval::<(), _>(script).map_err(|e| {
                    let caught = ctx.catch();
                    DiagramError::Engine(format!("{e}: {caught:?}"))
                })?;
            }
            Ok(())
        })?;
        Ok(Self {
            _runtime: runtime,
            context,
            deadline,
        })
    }

    fn render(&self, source: &str, dark: bool, timeout: Duration) -> Result<String, DiagramError> {
        let failed = |e: rquickjs::Error| DiagramError::Engine(e.to_string());
        *self.deadline.lock().expect("deadline") = Some(Instant::now() + timeout);
        let result = self
            .context
            .with(|ctx: Ctx| -> Result<String, DiagramError> {
                let globals = ctx.globals();
                ctx.eval::<(), _>("__out = null; __err = null;")
                    .map_err(failed)?;
                let lines: Vec<String> = source.lines().map(str::to_owned).collect();
                globals.set("__lines", lines).map_err(failed)?;
                let call = format!(
                    "__plantuml.renderToString(__lines, function (s) {{ __out = s; }}, \
                 function (e) {{ __err = String(e); }}, {{ dark: {dark} }});"
                );
                let interrupted = |ctx: &Ctx, e: rquickjs::Error| {
                    let caught = ctx.catch();
                    let message = format!("{e} {caught:?}");
                    if message.contains("interrupted") {
                        DiagramError::TimedOut(timeout.as_secs())
                    } else {
                        DiagramError::Engine(message)
                    }
                };
                ctx.eval::<(), _>(call).map_err(|e| interrupted(&ctx, e))?;
                loop {
                    while ctx.execute_pending_job() {}
                    let stepped: bool = ctx.eval("__step()").map_err(|e| interrupted(&ctx, e))?;
                    if !stepped {
                        while ctx.execute_pending_job() {}
                        let more: bool = ctx.eval("__step()").map_err(|e| interrupted(&ctx, e))?;
                        if !more {
                            break;
                        }
                    }
                }
                let out: Option<String> = globals.get("__out").map_err(failed)?;
                let err: Option<String> = globals.get("__err").map_err(failed)?;
                match (out, err) {
                    (Some(svg), _) => Ok(svg),
                    (None, Some(message)) => Err(engine_error(&message)),
                    (None, None) => Err(DiagramError::Engine("no result".into())),
                }
            });
        *self.deadline.lock().expect("deadline") = None;
        result
    }
}

impl DiagramRenderer for PlantUml {
    fn render(&mut self, source: &str, dark: bool) -> Result<String, DiagramError> {
        let timeout = self.timeout;
        let result = self.engine()?.render(source, dark, timeout);
        // After a stopped or failed render the engine starts afresh.
        if matches!(
            result,
            Err(DiagramError::TimedOut(_) | DiagramError::Engine(_))
        ) {
            self.engine = None;
        }
        let svg = result?;
        match syntax_error(&svg) {
            Some(error) => Err(error),
            None => Ok(svg),
        }
    }
}

fn engine_error(message: &str) -> DiagramError {
    let message = message
        .trim_start_matches("java.lang.RuntimeException: ")
        .to_owned();
    if message.contains("maxSvgSize") || message.contains("too large") {
        DiagramError::TooLarge
    } else {
        DiagramError::Engine(message)
    }
}

/// PlantUML answers a source with an error with a picture of the error;
/// UC-004 A11 shows the source with the error instead.
fn syntax_error(svg: &str) -> Option<DiagramError> {
    if !svg.contains("[From textarea (line ") {
        return None;
    }
    let texts: Vec<String> = svg
        .split("<text")
        .skip(1)
        .filter_map(|t| {
            let start = t.find('>')? + 1;
            let end = t.find("</text>")?;
            Some(unescape(&t[start..end]))
        })
        .collect();
    let line = texts.iter().find_map(|t| {
        let rest = t.split("(line ").nth(1)?;
        rest.split(')').next()?.trim().parse::<usize>().ok()
    });
    let message = texts
        .iter()
        .rev()
        .find(|t| !t.trim().is_empty())
        .map_or("Syntax error", |t| t.trim())
        .to_owned();
    Some(DiagramError::Syntax { line, message })
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// The width of `text` in DejaVu Sans at `size` pixels, the font the SVG is
/// rasterized with.
fn measure(text: String, size: f64, bold: bool) -> f64 {
    let data = if bold {
        dejavu::sans::bold()
    } else {
        dejavu::sans::regular()
    };
    let Ok(font) = FontRef::try_from_slice(data) else {
        return text.chars().count() as f64 * size * 0.6;
    };
    let scaled = font.as_scaled(PxScale::from(size as f32));
    text.chars()
        .map(|c| f64::from(scaled.h_advance(scaled.glyph_id(c))))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TC-004-04, ADR-009: the five diagram types render offline, light and
    /// dark.
    #[test]
    fn uc004_tc04_plantuml_renders_offline() {
        let mut plantuml = PlantUml::new();
        let diagrams = [
            "@startuml\nAlice -> Bob : hello\n@enduml",
            "@startuml\nclass Note\nclass Workspace\nWorkspace *-- Note\n@enduml",
            "@startuml\nstart\n:Open;\nif (exists?) then (yes)\n:Show;\nendif\nstop\n@enduml",
            "@startuml\n[*] --> Clean\nClean --> Dirty : edit\n@enduml",
            "@startuml\n[App] --> [Core]\n@enduml",
        ];
        for source in diagrams {
            for dark in [false, true] {
                let svg = plantuml.render(source, dark).unwrap();
                assert!(svg.starts_with("<svg"), "{source}");
                assert!(svg.contains("</svg>"));
            }
        }
        // Without @startuml it works as well.
        assert!(plantuml.render("Alice -> Bob", false).is_ok());
    }

    /// TC-004-23: a source with an error gives the error and its line.
    #[test]
    fn uc004_tc23_plantuml_syntax_error() {
        let mut plantuml = PlantUml::new();
        let error = plantuml
            .render("@startuml\nclass A {\nA -> -> B\n@enduml", false)
            .unwrap_err();
        match error {
            DiagramError::Syntax { line, message } => {
                assert_eq!(line, Some(2));
                assert!(message.contains("Syntax Error"), "{message}");
            }
            other => panic!("{other:?}"),
        }
        // The engine renders correct diagrams afterwards.
        assert!(plantuml.render("Alice -> Bob", false).is_ok());
    }

    /// ADR-009: a render that takes too long is stopped, and the engine
    /// starts afresh for the next diagram.
    #[test]
    fn adr009_plantuml_render_times_out() {
        let mut plantuml = PlantUml::with_timeout(Duration::from_millis(1));
        let mut source = String::from("@startuml\n");
        for i in 0..60 {
            source.push_str(&format!("class C{i}\nC{} <|-- C{i}\n", i / 2));
        }
        source.push_str("@enduml");
        assert!(matches!(
            plantuml.render(&source, false),
            Err(DiagramError::TimedOut(_))
        ));
        plantuml.timeout = TIMEOUT;
        assert!(plantuml.render("Alice -> Bob", false).is_ok());
    }
}
