//! Loading the images of a note for the facade (UC-004 step 3, BR-004): a
//! file of this computer, or a web address loaded without sending anything
//! of the note (NFR-015). Images load on a worker thread with a cache, the
//! most recently asked for first, like diagrams (ADR-009).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crate::diagram::{rasterize, Picture};

/// Images kept in the cache.
const CACHE_SIZE: usize = 128;
/// An image nobody asked for again within this time is not loaded.
const FORGET_AFTER: Duration = Duration::from_secs(3);
/// Largest image file loaded, from disk or the web.
pub const MAX_BYTES: u64 = 50 * 1024 * 1024;
/// Largest side of a picture in pixels; larger images are scaled down.
const MAX_SIDE: u32 = 4096;
/// How long loading a web image may take.
const WEB_TIMEOUT: Duration = Duration::from_secs(15);

/// Where an image is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageSource {
    File(PathBuf),
    Web(String),
}

/// Why an image cannot be shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    #[error("{} was not found", .0.display())]
    NotFound(PathBuf),
    /// A web image could not be loaded, for example without a network
    /// connection (UC-004 BR-004).
    #[error("{0} could not be loaded")]
    NotLoaded(String),
    #[error("the image is larger than {} MB", MAX_BYTES / (1024 * 1024))]
    TooLarge,
    #[error("the image cannot be read: {0}")]
    Unreadable(String),
}

/// What is known of an image.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageState {
    Pending,
    Ready(Arc<Picture>),
    Failed(ImageError),
}

/// The images of the app, loaded on a worker thread.
pub struct Images {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<State>,
    work: Condvar,
    wake: Box<dyn Fn() + Send + Sync>,
}

#[derive(Default)]
struct State {
    cache: HashMap<Key, Done>,
    wanted: HashMap<Key, Instant>,
    generation: u64,
    stop: bool,
}

/// An image at a scale; a file also by its time of change, so an image
/// changed on disk is loaded again.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    source: ImageSource,
    changed: Option<SystemTime>,
    scale: u32,
}

struct Done {
    result: Result<Arc<Picture>, ImageError>,
    used: Instant,
}

impl Images {
    /// Starts the worker thread; `wake` is called when an image is done.
    pub fn start(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            work: Condvar::new(),
            wake: Box::new(wake),
        });
        let worker = Arc::clone(&shared);
        thread::Builder::new()
            .name("underline-images".into())
            .spawn(move || work(&worker))
            .expect("start the image thread");
        Self { shared }
    }

    /// The image at `source`, as pixels at `scale` pixels per point.
    pub fn get(&self, source: &ImageSource, scale: f32) -> ImageState {
        let changed = match source {
            ImageSource::File(path) => match std::fs::metadata(path) {
                Ok(meta) => meta.modified().ok(),
                Err(_) => return ImageState::Failed(ImageError::NotFound(path.clone())),
            },
            ImageSource::Web(_) => None,
        };
        let key = Key {
            source: source.clone(),
            changed,
            scale: scale.to_bits(),
        };
        let mut state = self.shared.state.lock().expect("image state");
        if let Some(done) = state.cache.get_mut(&key) {
            done.used = Instant::now();
            return match &done.result {
                Ok(picture) => ImageState::Ready(Arc::clone(picture)),
                Err(error) => ImageState::Failed(error.clone()),
            };
        }
        state.wanted.insert(key, Instant::now());
        self.shared.work.notify_one();
        ImageState::Pending
    }

    /// Counts the images loaded so far; it changes when one is done.
    pub fn generation(&self) -> u64 {
        self.shared.state.lock().expect("image state").generation
    }
}

impl Drop for Images {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.stop = true;
        }
        self.shared.work.notify_all();
    }
}

fn work(shared: &Shared) {
    loop {
        let key = {
            let mut state = shared.state.lock().expect("image state");
            loop {
                if state.stop {
                    return;
                }
                state
                    .wanted
                    .retain(|_, asked| asked.elapsed() < FORGET_AFTER);
                let latest = state
                    .wanted
                    .iter()
                    .max_by_key(|(_, asked)| **asked)
                    .map(|(key, _)| key.clone());
                if let Some(key) = latest {
                    state.wanted.remove(&key);
                    break key;
                }
                state = shared.work.wait(state).expect("image state");
            }
        };
        let scale = f32::from_bits(key.scale);
        let result = load(&key.source).and_then(|bytes| decode(&key.source, &bytes, scale));
        {
            let mut state = shared.state.lock().expect("image state");
            if state.cache.len() >= CACHE_SIZE {
                let oldest = state
                    .cache
                    .iter()
                    .min_by_key(|(_, done)| done.used)
                    .map(|(key, _)| key.clone());
                if let Some(oldest) = oldest {
                    state.cache.remove(&oldest);
                }
            }
            state.cache.insert(
                key,
                Done {
                    result: result.map(Arc::new),
                    used: Instant::now(),
                },
            );
            state.generation += 1;
        }
        (shared.wake)();
    }
}

/// The bytes of the image.
fn load(source: &ImageSource) -> Result<Vec<u8>, ImageError> {
    match source {
        ImageSource::File(path) => {
            let size = std::fs::metadata(path)
                .map_err(|_| ImageError::NotFound(path.clone()))?
                .len();
            if size > MAX_BYTES {
                return Err(ImageError::TooLarge);
            }
            std::fs::read(path).map_err(|e| ImageError::Unreadable(e.to_string()))
        }
        ImageSource::Web(address) => load_web(address),
    }
}

/// NFR-015: a plain request for the address, with no cookies and nothing
/// of the note; the server learns only the address and that Underline asks.
fn load_web(address: &str) -> Result<Vec<u8>, ImageError> {
    let not_loaded = || ImageError::NotLoaded(address.to_owned());
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(WEB_TIMEOUT))
        .user_agent("Underline")
        .build()
        .into();
    let response = agent.get(address).call().map_err(|_| not_loaded())?;
    let mut bytes = Vec::new();
    response
        .into_body()
        .into_reader()
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| not_loaded())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(ImageError::TooLarge);
    }
    Ok(bytes)
}

/// Pixels of the image: SVG is rasterized at `scale`, other formats are
/// decoded at their size, one image pixel to one point.
fn decode(source: &ImageSource, bytes: &[u8], scale: f32) -> Result<Picture, ImageError> {
    let svg = match source {
        ImageSource::File(path) => has_extension(path, "svg"),
        ImageSource::Web(address) => address
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .ends_with(".svg"),
    } || bytes.starts_with(b"<svg")
        || bytes.starts_with(b"<?xml");
    if svg {
        let text = std::str::from_utf8(bytes).map_err(|e| ImageError::Unreadable(e.to_string()))?;
        return rasterize(text, scale).map_err(|e| ImageError::Unreadable(e.to_string()));
    }
    let image =
        image::load_from_memory(bytes).map_err(|e| ImageError::Unreadable(e.to_string()))?;
    let (width, height) = (image.width(), image.height());
    let image = if width.max(height) > MAX_SIDE {
        image.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    // Pixels per point: the shown size stays that of the original image.
    let shrink = image.width() as f32 / width as f32;
    let mut rgba = image.into_rgba8();
    for pixel in rgba.pixels_mut() {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Ok(Picture {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
        scale: shrink,
    })
}

/// UC-005 A18: an image from the clipboard (RGBA, not premultiplied) as
/// PNG bytes.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, ImageError> {
    let mut png = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png);
    image::ImageEncoder::write_image(
        encoder,
        rgba,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| ImageError::Unreadable(e.to_string()))?;
    Ok(png)
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(images: &Images, source: &ImageSource) -> ImageState {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match images.get(source, 2.0) {
                ImageState::Pending if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10))
                }
                state => return state,
            }
        }
    }

    /// TC-004-06: a local image is shown; a missing one and a web image that
    /// cannot be loaded give a reason that names them.
    #[test]
    fn uc004_tc06_loads_images() {
        let dir = std::env::temp_dir().join(format!("underline-images-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut png = image::RgbaImage::new(30, 20);
        for pixel in png.pixels_mut() {
            *pixel = image::Rgba([0, 0, 255, 255]);
        }
        let file = dir.join("local.png");
        png.save(&file).unwrap();
        std::fs::write(
            dir.join("shape.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="8"><rect width="10" height="8"/></svg>"#,
        )
        .unwrap();

        let images = Images::start(|| {});
        let ImageState::Ready(picture) = wait(&images, &ImageSource::File(file.clone())) else {
            panic!("no picture");
        };
        assert_eq!((picture.width, picture.height), (30, 20));
        assert_eq!(picture.size(), [30.0, 20.0]);
        assert_eq!(&picture.pixels[..4], &[0, 0, 255, 255]);

        let ImageState::Ready(svg) = wait(&images, &ImageSource::File(dir.join("shape.svg")))
        else {
            panic!("no svg");
        };
        assert_eq!((svg.width, svg.height), (20, 16));

        let missing = dir.join("missing.png");
        assert_eq!(
            images.get(&ImageSource::File(missing.clone()), 1.0),
            ImageState::Failed(ImageError::NotFound(missing))
        );
        // No web server answers on this port: the address is named.
        let address = "http://127.0.0.1:9/picture.png".to_owned();
        assert_eq!(
            wait(&images, &ImageSource::Web(address.clone())),
            ImageState::Failed(ImageError::NotLoaded(address))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
