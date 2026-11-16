//! FR-098, UC-004 BR-019, OQ-222, ADR-029: AsciiMath written as LaTeX, so
//! the formulas of an AsciiDoc note written in AsciiMath (`asciimath:[…]`,
//! `stem:[…]` while the note's `stem` is AsciiMath, `[asciimath]` blocks)
//! are typeset by the same engine as LaTeX ones and exported as LaTeX.
//!
//! The AsciiMath is read by the `asciimath-parser` crate (MIT), whose tree
//! keeps every token as written; this module writes that tree as LaTeX:
//!
//! - symbols by the table of AsciiMath (asciimath.org), their Unicode
//!   forms as their spelling;
//! - fractions (`a/b`, `frac a b`), sub- and superscripts, roots, the
//!   accents, the fonts and `text(…)`, `"…"`, colours;
//! - brackets as stretching delimiters, `{: … :}` invisible; the brackets
//!   around a fraction's parts, a script, and the argument of `sqrt` and
//!   its kin are dropped, as AsciiMath drops them;
//! - matrices (`[(a, b), (c, d)]`) as a `matrix` within their brackets, with
//!   the column lines of `[(a, |, b)]` as an `array`.
//!
//! AsciiMath has no errors: whatever is written is read as something, so the
//! LaTeX is always whole; what the engine cannot typeset says why (UC-004
//! A21).
//!
//! Bounds: the parser nests at most 256 levels deep (what lies deeper is
//! read as missing), so writing its tree recurses as deep at most, and it
//! reads a source in linear time. The LaTeX is a few times as long as the
//! AsciiMath; the typesetting refuses a source beyond
//! `underline_facade::math::MAX_SOURCE_BYTES` (16 KiB) with its reason.

use std::fmt::Write as _;

use asciimath_parser::tree::{
    Expression, Frac, Func, Group, Intermediate, Matrix, Script, ScriptFunc, Signed, Simple,
    SimpleBinary, SimpleScript, SimpleUnary,
};
use asciimath_parser::ASCIIMATH_TOKENS;

/// FR-098, OQ-222: the AsciiMath formula `source` written in LaTeX.
///
/// ```
/// use underline_markup::asciimath::to_latex;
///
/// assert_eq!(to_latex("sum_(i=1)^n i^2"), r"\sum_{i=1}^{n} i^{2}");
/// assert_eq!(to_latex("(a+b)/2"), r"\frac{a+b}{2}");
/// ```
#[must_use]
pub fn to_latex(source: &str) -> String {
    let expression = asciimath_parser::parse(source);
    let mut out = Latex::default();
    out.expression(&expression);
    out.text.trim().to_owned()
}

/// The LaTeX written so far.
#[derive(Default)]
struct Latex {
    text: String,
}

impl Latex {
    /// Writes `piece`, with a space after a control word that a letter
    /// would otherwise join (`\alpha x`, not `\alphax`).
    fn push(&mut self, piece: &str) {
        let joins = piece.starts_with(|c: char| c.is_ascii_alphabetic()) && self.ends_in_word();
        if joins {
            self.text.push(' ');
        }
        self.text.push_str(piece);
    }

    /// Whether the text ends in a control word, `\` and letters.
    fn ends_in_word(&self) -> bool {
        let letters = self
            .text
            .chars()
            .rev()
            .take_while(char::is_ascii_alphabetic)
            .count();
        letters > 0 && self.text[..self.text.len() - letters].ends_with('\\')
    }

    fn expression(&mut self, expression: &Expression<'_>) {
        for part in expression.iter() {
            match part {
                Intermediate::ScriptFunc(func) => self.script_func(func),
                Intermediate::Frac(frac) => self.frac(frac),
                // Space between operands is LaTeX's to set; one keeps two
                // names apart.
                Intermediate::Space(_) => {
                    if !self.text.ends_with(' ') && !self.text.is_empty() {
                        self.text.push(' ');
                    }
                }
            }
        }
    }

    fn frac(&mut self, frac: &Frac<'_>) {
        self.push("\\frac{");
        self.bare_script_func(&frac.numer);
        self.push("}{");
        self.bare_script_func(&frac.denom);
        self.push("}");
    }

    fn script_func(&mut self, func: &ScriptFunc<'_>) {
        match func {
            ScriptFunc::Simple(simple) => self.simple_script(simple),
            ScriptFunc::Func(func) => self.func(func),
            ScriptFunc::Signed(signed) => self.signed(signed),
        }
    }

    /// A fraction's part, without the brackets around it.
    fn bare_script_func(&mut self, func: &ScriptFunc<'_>) {
        match func {
            ScriptFunc::Simple(SimpleScript {
                simple,
                script: Script::None,
            }) => self.bare(simple),
            other => self.script_func(other),
        }
    }

    fn signed(&mut self, signed: &Signed<'_>) {
        self.push(sign(signed.sign));
        self.script_func(&signed.operand);
    }

    fn simple_script(&mut self, simple: &SimpleScript<'_>) {
        self.simple(&simple.simple);
        self.script(&simple.script);
    }

    fn script(&mut self, script: &Script<'_>) {
        if let Some(sub) = script.sub() {
            self.push("_{");
            self.bare(sub);
            self.push("}");
        }
        if let Some(sup) = script.sup() {
            self.push("^{");
            self.bare(sup);
            self.push("}");
        }
    }

    fn func(&mut self, func: &Func<'_>) {
        self.push(&function(func.func));
        self.script(&func.script);
        self.function_argument(func.arg());
    }

    /// The argument of a function such as `sin`: its brackets stay.
    fn function_argument(&mut self, arg: &ScriptFunc<'_>) {
        let missing = matches!(
            arg,
            ScriptFunc::Simple(SimpleScript {
                simple: Simple::Missing,
                script: Script::None,
            })
        );
        if !missing {
            self.text.push(' ');
            self.script_func(arg);
        }
    }

    /// `simple`, without the brackets around it: the argument of `sqrt`,
    /// a script, a fraction's part.
    fn bare(&mut self, simple: &Simple<'_>) {
        match simple {
            Simple::Group(group) if droppable(group) => self.expression(&group.expr),
            other => self.simple(other),
        }
    }

    /// `simple` in braces, without the brackets around it.
    fn braced(&mut self, simple: &Simple<'_>) {
        self.push("{");
        self.bare(simple);
        self.push("}");
    }

    fn simple(&mut self, simple: &Simple<'_>) {
        match simple {
            Simple::Missing => {}
            Simple::Number(number) => self.push(number),
            Simple::Text(text) => self.text_command(text),
            Simple::Ident(ident) | Simple::Operator(ident) => self.push(&literal(ident)),
            Simple::Symbol(symbol) => self.push(&self::symbol(symbol.text)),
            Simple::Sign(text) => self.push(sign(text)),
            Simple::Signed(signed) => {
                self.push(sign(signed.sign));
                self.simple(signed.operand());
            }
            Simple::Unary(unary) => self.unary(unary),
            Simple::Func(func) => {
                self.push(&function(func.func));
                if func.arg().as_option().is_some() {
                    self.text.push(' ');
                    self.simple(func.arg());
                }
            }
            Simple::Binary(binary) => self.binary(binary),
            Simple::Group(group) => self.group(group),
            Simple::Matrix(matrix) => self.matrix(matrix),
        }
    }

    /// `"…"` and `text(…)`: the text as it is, in the text font.
    fn text_command(&mut self, text: &str) {
        self.push("\\text{");
        self.text.push_str(&escape_text(text));
        self.push("}");
    }

    fn unary(&mut self, unary: &SimpleUnary<'_>) {
        let op = canonical(unary.op);
        if matches!(op, "text" | "mbox") {
            let raw = raw_text(unary.arg());
            return self.text_command(&raw);
        }
        let (open, close) = match op {
            "sqrt" => ("\\sqrt{", "}"),
            "abs" | "Abs" => ("\\left|", "\\right|"),
            "norm" => ("\\left\\|", "\\right\\|"),
            "floor" => ("\\left\\lfloor ", "\\right\\rfloor"),
            "ceil" => ("\\left\\lceil ", "\\right\\rceil"),
            "hat" => ("\\hat{", "}"),
            "bar" | "overline" => ("\\overline{", "}"),
            "vec" => ("\\vec{", "}"),
            "dot" => ("\\dot{", "}"),
            "ddot" => ("\\ddot{", "}"),
            "overarc" | "overparen" => ("\\overgroup{", "}"),
            "ul" | "underline" => ("\\underline{", "}"),
            "ubrace" | "underbrace" => ("\\underbrace{", "}"),
            "obrace" | "overbrace" => ("\\overbrace{", "}"),
            "cancel" => ("\\cancel{", "}"),
            "tilde" => ("\\tilde{", "}"),
            "bb" | "mathbf" => ("\\mathbf{", "}"),
            "sf" | "mathsf" => ("\\mathsf{", "}"),
            "bbb" | "mathbb" => ("\\mathbb{", "}"),
            "cc" | "mathcal" => ("\\mathcal{", "}"),
            "tt" | "mathtt" => ("\\mathtt{", "}"),
            "fr" | "mathfrak" => ("\\mathfrak{", "}"),
            "it" | "mathit" => ("\\mathit{", "}"),
            "bbit" => ("\\boldsymbol{\\mathit{", "}}"),
            "bbsf" => ("\\boldsymbol{\\mathsf{", "}}"),
            "sfit" => ("\\mathsf{\\mathit{", "}}"),
            "bbsfit" => ("\\boldsymbol{\\mathsf{\\mathit{", "}}}"),
            "bbcc" => ("\\boldsymbol{\\mathcal{", "}}"),
            "bbfr" => ("\\boldsymbol{\\mathfrak{", "}}"),
            other => {
                // A unary operator of another token map: its name, then
                // its argument.
                self.push(&literal(other));
                return self.simple(unary.arg());
            }
        };
        self.push(open);
        self.bare(unary.arg());
        self.push(close);
    }

    fn binary(&mut self, binary: &SimpleBinary<'_>) {
        match canonical(binary.op) {
            "frac" => {
                self.push("\\frac");
                self.braced(binary.first());
                self.braced(binary.second());
            }
            "root" => {
                // The index in braces, so a `]` in it does not end it.
                self.push("\\sqrt[{");
                self.bare(binary.first());
                self.push("}]");
                self.braced(binary.second());
            }
            "stackrel" | "overset" => {
                self.push("\\overset");
                self.braced(binary.first());
                self.braced(binary.second());
            }
            "underset" => {
                self.push("\\underset");
                self.braced(binary.first());
                self.braced(binary.second());
            }
            "color" => {
                let name = color_name(&raw_text(binary.first()));
                if name.is_empty() {
                    self.bare(binary.second());
                } else {
                    let _ = write!(self.text, "\\textcolor{{{name}}}");
                    self.braced(binary.second());
                }
            }
            // `id` and `class` name the part for HTML; the part stays.
            "id" | "class" => self.bare(binary.second()),
            other => {
                self.push(&literal(other));
                self.simple(binary.first());
                self.simple(binary.second());
            }
        }
    }

    fn group(&mut self, group: &Group<'_>) {
        let (left, right) = (bracket(group.left_bracket), bracket(group.right_bracket));
        if left == "." && right == "." {
            self.push("{");
            self.expression(&group.expr);
            self.push("}");
            return;
        }
        self.push("\\left");
        self.push(left);
        self.text.push(' ');
        self.expression(&group.expr);
        self.push("\\right");
        self.push(right);
    }

    fn matrix(&mut self, matrix: &Matrix<'_>) {
        let (left, right) = (bracket(matrix.left_bracket), bracket(matrix.right_bracket));
        let lines = matrix.column_lines();
        let columns = matrix.num_cols();
        self.push("\\left");
        self.push(left);
        if lines.is_empty() {
            self.push("\\begin{matrix}");
        } else {
            let mut spec = String::new();
            for column in 0..=columns {
                for _ in lines.iter().filter(|&&line| line == column) {
                    spec.push('|');
                }
                if column < columns {
                    spec.push('c');
                }
            }
            let _ = write!(self.text, "\\begin{{array}}{{{spec}}}");
        }
        for (row, cells) in matrix.rows().enumerate() {
            if row > 0 {
                self.push(" \\\\ ");
            }
            for (column, cell) in cells.iter().enumerate() {
                if column > 0 {
                    self.push(" & ");
                }
                self.expression(cell);
            }
        }
        self.push(if lines.is_empty() {
            "\\end{matrix}"
        } else {
            "\\end{array}"
        });
        self.push("\\right");
        self.push(right);
    }
}

/// The colour named in `text` as LaTeX takes it: letters and digits, or `#`
/// and hex digits; empty when it names none.
fn color_name(text: &str) -> String {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix('#') {
        return if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            text.to_owned()
        } else {
            String::new()
        };
    }
    text.chars().filter(char::is_ascii_alphanumeric).collect()
}

/// Whether the brackets of `group` are dropped around a fraction's part, a
/// script or an argument: round, square or curly ones, or invisible ones.
fn droppable(group: &Group<'_>) -> bool {
    matches!(
        canonical(group.left_bracket),
        "(" | "[" | "{" | "{:" | "left(" | "left["
    ) && matches!(
        canonical(group.right_bracket),
        ")" | "]" | "}" | ":}" | "right)" | "right]"
    )
}

/// The text a token stands for, as typed: a group's inner text, the
/// characters of names and numbers (`color(red)`, `text(…)`).
fn raw_text(simple: &Simple<'_>) -> String {
    fn expression(expression: &Expression<'_>, out: &mut String) {
        for part in expression.iter() {
            match part {
                Intermediate::ScriptFunc(func) => script_func(func, out),
                Intermediate::Space(space) => out.push_str(space),
                Intermediate::Frac(_) => {}
            }
        }
    }
    // `f` and `g` are functions of AsciiMath: `#f0a` holds one.
    fn script_func(func: &ScriptFunc<'_>, out: &mut String) {
        match func {
            ScriptFunc::Simple(simple) => raw(&simple.simple, out),
            ScriptFunc::Func(func) => {
                out.push_str(func.func);
                script_func(func.arg(), out);
            }
            ScriptFunc::Signed(signed) => {
                out.push_str(signed.sign);
                script_func(&signed.operand, out);
            }
        }
    }
    fn raw(simple: &Simple<'_>, out: &mut String) {
        match simple {
            Simple::Number(text)
            | Simple::Text(text)
            | Simple::Ident(text)
            | Simple::Operator(text)
            | Simple::Sign(text) => out.push_str(text),
            Simple::Symbol(symbol) => out.push_str(symbol.text),
            Simple::Group(group) => expression(&group.expr, out),
            Simple::Func(func) => {
                out.push_str(func.func);
                raw(func.arg(), out);
            }
            _ => {}
        }
    }
    let mut out = String::new();
    raw(simple, &mut out);
    out
}

/// The spelling of `text`: a Unicode form AsciiMath reads (`⊂`) stands for
/// the spelling it renders (`subset`).
fn canonical(text: &str) -> &str {
    if ASCIIMATH_TOKENS.iter().any(|token| token.spelling == text) {
        return text;
    }
    ASCIIMATH_TOKENS
        .iter()
        .find(|token| token.unicode == Some(text))
        .map_or(text, |token| token.spelling)
}

/// A sign: `+`, `-`, `+-`, `-+`.
fn sign(text: &str) -> &'static str {
    match canonical(text) {
        "+" => "+",
        "+-" | "pm" => "\\pm ",
        "-+" | "mp" => "\\mp ",
        _ => "-",
    }
}

/// A function of AsciiMath: LaTeX's command where it has one.
fn function(name: &str) -> String {
    match name {
        "sin" | "cos" | "tan" | "sinh" | "cosh" | "tanh" | "cot" | "sec" | "csc" | "arcsin"
        | "arccos" | "arctan" | "coth" | "exp" | "log" | "ln" | "det" | "gcd" => {
            format!("\\{name}")
        }
        "f" | "g" => name.to_owned(),
        other => format!("\\operatorname{{{}}}", escape_text(other)),
    }
}

/// A bracket as a LaTeX delimiter; `.` for none.
fn bracket(text: &str) -> &'static str {
    match canonical(text) {
        "(" | "left(" => "(",
        ")" | "right)" => ")",
        "[" | "left[" => "[",
        "]" | "right]" => "]",
        "{" => "\\{",
        "}" => "\\}",
        "|:" | ":|" | "|" => "|",
        "||" => "\\|",
        "(:" | "<<" | "langle" => "\\langle",
        ":)" | ">>" | "rangle" => "\\rangle",
        "|__" | "lfloor" => "\\lfloor",
        "__|" | "rfloor" => "\\rfloor",
        "|~" | "lceiling" => "\\lceil",
        "~|" | "rceiling" => "\\rceil",
        _ => ".",
    }
}

/// A character outside the table as LaTeX reads it in a formula.
fn literal(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\backslash "),
            '{' | '}' | '#' | '$' | '%' | '&' | '_' => {
                out.push('\\');
                out.push(c);
            }
            '^' => out.push_str("\\hat{}"),
            '~' => out.push_str("\\sim "),
            _ => out.push(c),
        }
    }
    out
}

/// Text in `\text{…}`: LaTeX's special characters escaped.
fn escape_text(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' | '}' | '#' | '$' | '%' | '&' | '_' => {
                out.push('\\');
                out.push(c);
            }
            '^' => out.push_str("\\textasciicircum{}"),
            '~' => out.push_str("\\textasciitilde{}"),
            _ => out.push(c),
        }
    }
    out
}

/// A symbol of AsciiMath in LaTeX (asciimath.org's table).
fn symbol(text: &str) -> String {
    let spelling = canonical(text);
    let latex = match spelling {
        // Greek letters; the capitals LaTeX has no command for are Latin.
        "alpha" | "beta" | "chi" | "delta" | "Delta" | "eta" | "gamma" | "Gamma" | "iota"
        | "kappa" | "varkappa" | "lambda" | "Lambda" | "mu" | "nu" | "omega" | "Omega" | "Phi"
        | "pi" | "Pi" | "varpi" | "psi" | "Psi" | "rho" | "varrho" | "sigma" | "Sigma" | "tau"
        | "theta" | "vartheta" | "Theta" | "upsilon" | "Upsilon" | "xi" | "Xi" | "zeta" => {
            return format!("\\{spelling}");
        }
        "lamda" => "\\lambda",
        "Lamda" => "\\Lambda",
        "Vartheta" => "\\varTheta",
        // AsciiMath's ε and φ are LaTeX's varepsilon and varphi.
        "epsi" | "epsilon" => "\\varepsilon",
        "varepsilon" => "\\epsilon",
        "phi" => "\\varphi",
        "varphi" => "\\phi",
        "Alpha" => "A",
        "Beta" => "B",
        "Chi" => "X",
        "Epsi" | "Epsilon" => "E",
        "Eta" => "H",
        "Iota" => "I",
        "Kappa" => "K",
        "Mu" => "M",
        "Nu" => "N",
        "Rho" => "P",
        "Tau" => "T",
        "Zeta" => "Z",
        // Operations.
        "*" | "cdot" => "\\cdot",
        "**" | "ast" => "\\ast",
        "***" | "star" => "\\star",
        "//" => "/",
        "\\\\" | "backslash" => "\\backslash",
        "setminus" => "\\setminus",
        "xx" | "times" => "\\times",
        "|><" | "ltimes" => "\\ltimes",
        "><|" | "rtimes" => "\\rtimes",
        "|><|" | "bowtie" => "\\bowtie",
        "-:" | "div" | "divide" => "\\div",
        "@" | "circ" => "\\circ",
        "o+" | "oplus" => "\\oplus",
        "ox" | "otimes" => "\\otimes",
        "o." | "odot" => "\\odot",
        "^^" | "wedge" | "land" => "\\wedge",
        "vv" | "vee" | "lor" => "\\vee",
        "nn" | "cap" => "\\cap",
        "uu" | "cup" => "\\cup",
        "o-" | "ominus" => "\\ominus",
        "oslash" => "\\oslash",
        "dag" | "dagger" => "\\dagger",
        "ddag" | "ddagger" => "\\ddagger",
        "sum" => "\\sum",
        "prod" => "\\prod",
        "^^^" | "bigwedge" => "\\bigwedge",
        "vvv" | "bigvee" => "\\bigvee",
        "nnn" | "bigcap" => "\\bigcap",
        "uuu" | "bigcup" => "\\bigcup",
        // Relations.
        "=" => "=",
        "!=" | "ne" => "\\ne",
        ":=" => ":=",
        "<" | "lt" => "<",
        "<=" | "le" | "lt=" | "leq" => "\\le",
        ">" | "gt" => ">",
        "mlt" | "ll" => "\\ll",
        ">=" | "ge" | "gt=" | "geq" => "\\ge",
        "mgt" | "gg" => "\\gg",
        "-<" | "prec" | "-lt" => "\\prec",
        ">-" | "succ" => "\\succ",
        "-<=" | "preceq" => "\\preceq",
        ">-=" | "succeq" => "\\succeq",
        "in" => "\\in",
        "!in" | "notin" => "\\notin",
        "sub" | "subset" => "\\subset",
        "sup" | "supset" => "\\supset",
        "sube" | "subseteq" => "\\subseteq",
        "supe" | "supseteq" => "\\supseteq",
        "!sub" | "nsub" | "notsubset" => "\\not\\subset",
        "!sube" | "nsubseteq" | "notsubseteq" => "\\nsubseteq",
        "!sup" | "nsup" | "notsupset" => "\\not\\supset",
        "!supe" | "nsupseteq" | "notsupseteq" => "\\nsupseteq",
        "-=" | "equiv" => "\\equiv",
        "!-=" | "notequiv" => "\\not\\equiv",
        "~=" | "cong" => "\\cong",
        "~~" | "approx" => "\\approx",
        "~" | "sim" => "\\sim",
        "prop" | "propto" => "\\propto",
        // Logic.
        "and" => "\\text{ and }",
        "or" => "\\text{ or }",
        "if" => "\\text{ if }",
        "not" | "neg" => "\\neg",
        "AA" | "forall" => "\\forall",
        "EE" | "exists" => "\\exists",
        "!EE" | "notexists" => "\\nexists",
        "=>" | "implies" | "rArr" | "Rightarrow" | "==>" => "\\Rightarrow",
        "<=>" | "iff" | "hArr" | "Leftrightarrow" | "<==>" => "\\Leftrightarrow",
        "|--" | "vdash" => "\\vdash",
        "|==" | "models" => "\\models",
        "_|_" | "bot" => "\\bot",
        "TT" | "top" => "\\top",
        // Miscellaneous.
        ":|:" => "\\mid",
        ":." | "therefore" => "\\therefore",
        ":'" | "because" => "\\because",
        "diamond" => "\\diamond",
        "frown" => "\\frown",
        "int" => "\\int",
        "oint" => "\\oint",
        "iint" => "\\iint",
        "iiint" => "\\iiint",
        "oiint" => "\\oiint",
        "oiiint" => "\\oiiint",
        // A space typed as `\ `; `~` survives the trimmed end.
        "\\ " => "~",
        "quad" => "\\quad",
        "qquad" => "\\qquad",
        "enspace" => "\\enspace",
        "thinspace" => "\\,",
        "del" | "partial" => "\\partial",
        "grad" | "nabla" => "\\nabla",
        "O/" | "emptyset" => "\\emptyset",
        "oo" | "infty" => "\\infty",
        "aleph" => "\\aleph",
        "..." | "ldots" => "\\ldots",
        "/_" | "angle" => "\\angle",
        "/_\\" | "triangle" => "\\triangle",
        "'" => "'",
        "prime" => "\\prime",
        "cdots" => "\\cdots",
        "vdots" => "\\vdots",
        "ddots" => "\\ddots",
        "square" => "\\square",
        "CC" => "\\mathbb{C}",
        "NN" => "\\mathbb{N}",
        "QQ" => "\\mathbb{Q}",
        "RR" => "\\mathbb{R}",
        "ZZ" => "\\mathbb{Z}",
        "ell" => "\\ell",
        "hbar" => "\\hbar",
        "!" => "!",
        "%" => "\\%",
        "," => ",",
        "lim" => "\\lim",
        "dim" => "\\dim",
        "min" => "\\min",
        "max" => "\\max",
        "Lim" | "lub" | "glb" => return format!("\\operatorname*{{{spelling}}}"),
        "mod" => "\\operatorname{mod}",
        "dx" | "dy" | "dz" | "dt" => return format!("\\mathrm{{d}}{}", &spelling[1..]),
        // Arrows.
        "uarr" | "uparrow" => "\\uparrow",
        "uArr" | "Uparrow" => "\\Uparrow",
        "darr" | "downarrow" => "\\downarrow",
        "dArr" | "Downarrow" => "\\Downarrow",
        "rarr" | "rightarrow" | "->" | "to" => "\\to",
        ">->" | "rightarrowtail" => "\\rightarrowtail",
        "->>" | "twoheadrightarrow" => "\\twoheadrightarrow",
        ">->>" | "twoheadrightarrowtail" => "\\twoheadrightarrow",
        "|->" | "mapsto" => "\\mapsto",
        "larr" | "leftarrow" | "<-" => "\\leftarrow",
        "harr" | "leftrightarrow" | "<->" => "\\leftrightarrow",
        "lArr" | "Leftarrow" | "<==" => "\\Leftarrow",
        "rightleftharpoons" => "\\rightleftharpoons",
        // `/`, `^` and `_` that stand alone (`x^`), and brackets read as
        // symbols because they close nothing.
        "/" => "/",
        "^" => "\\hat{}",
        "_" => "\\_",
        other => {
            let bracket = bracket(other);
            if bracket == "." {
                return literal(other);
            }
            return bracket.to_owned();
        }
    };
    latex.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OQ-222, TC-004-38: the common forms of the AsciiMath spec.
    #[test]
    fn oq222_asciimath_forms_as_latex() {
        let cases = [
            ("x^2", "x^{2}"),
            ("a_i^j", "a_{i}^{j}"),
            ("x_(i+1)", "x_{i+1}"),
            ("(a+b)/2", r"\frac{a+b}{2}"),
            ("1/x^2", r"\frac{1}{x^{2}}"),
            ("frac(a)(b)", r"\frac{a}{b}"),
            ("sqrt x", r"\sqrt{x}"),
            ("sqrt(x+1)", r"\sqrt{x+1}"),
            ("root(3)(x)", r"\sqrt[{3}]{x}"),
            ("sum_(i=1)^n i", r"\sum_{i=1}^{n} i"),
            (
                "int_0^1 f(x) dx",
                r"\int_{0}^{1} f \left( x\right) \mathrm{d}x",
            ),
            ("lim_(x->oo) 1/x", r"\lim_{x\to\infty} \frac{1}{x}"),
            ("alpha + beta", r"\alpha + \beta"),
            ("a != b", r"a \ne b"),
            ("x in RR", r"x \in \mathbb{R}"),
            ("sin(x)", r"\sin \left( x\right)"),
            ("sin^2 x", r"\sin^{2} x"),
            ("abs(x)", r"\left|x\right|"),
            ("hat x", r"\hat{x}"),
            ("vec v", r"\vec{v}"),
            ("bb A", r"\mathbf{A}"),
            ("text(if) x", r"\text{if} x"),
            (r#""a b" c"#, r"\text{a b} c"),
            ("{: a :}", "{a}"),
            ("(: a, b :)", r"\left\langle a, b\right\rangle"),
            (
                "[(a,b),(c,d)]",
                r"\left[\begin{matrix}a & b \\ c & d\end{matrix}\right]",
            ),
            ("color(red)(x)", r"\textcolor{red}{x}"),
            ("-x^2", "-x^{2}"),
            ("a +- b", r"a \pm b"),
            ("a xx b", r"a \times b"),
            ("ubrace(a+b)_n", r"\underbrace{a+b}_{n}"),
            ("x ⊂ y", r"x \subset y"),
            ("epsilon phi", r"\varepsilon \varphi"),
        ];
        let wrong: Vec<_> = cases
            .iter()
            .filter(|(asciimath, latex)| to_latex(asciimath) != *latex)
            .map(|(asciimath, _)| (*asciimath, to_latex(asciimath)))
            .collect();
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// OQ-222: the column lines of a matrix make it an array.
    #[test]
    fn oq222_a_matrix_with_column_lines_is_an_array() {
        assert_eq!(
            to_latex("[(a, |, b), (c, |, d)]"),
            r"\left[\begin{array}{c|c}a & b \\ c & d\end{array}\right]"
        );
    }

    /// OQ-222: LaTeX's special characters stay characters; a control word
    /// is kept apart from the letter after it.
    #[test]
    fn oq222_special_characters_are_escaped() {
        assert_eq!(to_latex(r#""50% & #1""#), r"\text{50\% \& \#1}");
        assert_eq!(to_latex("a ? b"), "a ? b");
        assert_eq!(to_latex("alpha x"), r"\alpha x");
        assert_eq!(to_latex("alphax"), r"\alpha x");
        assert_eq!(to_latex(""), "");
    }

    /// OQ-222 (property): whatever is written gives LaTeX with balanced
    /// braces and `\left`/`\right` pairs, and never panics.
    #[test]
    fn oq222_any_text_gives_balanced_latex() {
        const PIECES: &[&str] = &[
            "(", ")", "[", "]", "{", "}", "{:", ":}", "|", "||", "/", "^", "_", ",", "x", "2",
            "sqrt", "frac", "root", "text", "\"t\"", "sum", "sin", " ", "-", "+-", "\\", "%", "(:",
            ":)", "color", "abs", "ubrace", "oo", "->", "'", "!", "&", "#", "~",
        ];
        let mut state = 7_u64;
        for _ in 0..3000 {
            let mut source = String::new();
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let n = (state >> 33) % 16;
            for _ in 0..n {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let pick = usize::try_from((state >> 33) % PIECES.len() as u64).unwrap_or(0);
                source.push_str(PIECES[pick]);
            }
            let latex = to_latex(&source);
            let mut depth = 0_i64;
            let mut escaped = false;
            for c in latex.chars() {
                match c {
                    _ if escaped => escaped = false,
                    '\\' => escaped = true,
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                assert!(depth >= 0, "{source:?} -> {latex:?}");
            }
            assert_eq!(depth, 0, "{source:?} -> {latex:?}");
            let delimiters = |word: &str| {
                latex
                    .match_indices(word)
                    .filter(|(at, _)| {
                        !latex[at + word.len()..].starts_with(|c: char| c.is_ascii_alphabetic())
                    })
                    .count()
            };
            assert_eq!(
                delimiters("\\left"),
                delimiters("\\right"),
                "{source:?} -> {latex:?}"
            );
            // The `[…]` of a root's index closes right after its braces,
            // whatever the index holds.
            for (at, _) in latex.match_indices("\\sqrt[") {
                let rest = &latex[at + 6..];
                assert!(rest.starts_with('{'), "{source:?} -> {latex:?}");
                let mut depth = 0_i64;
                let mut escaped = false;
                let mut close = None;
                for (i, c) in rest.char_indices() {
                    match c {
                        _ if escaped => escaped = false,
                        '\\' => escaped = true,
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        ']' if depth == 0 => {
                            close = Some(i);
                            break;
                        }
                        _ => {}
                    }
                }
                let close = close.unwrap_or_else(|| panic!("{source:?} -> {latex:?}"));
                assert!(rest[..close].ends_with('}'), "{source:?} -> {latex:?}");
            }
        }
    }

    /// OQ-222 (review): `color` takes a name or `#` with hex digits only;
    /// a root's index is braced, so a `]` in it stays inside.
    #[test]
    fn oq222_colours_and_root_indices() {
        assert_eq!(to_latex("color(#)(x)"), "x");
        assert_eq!(to_latex("color(#zz)(x)"), "x");
        assert_eq!(to_latex("color(#f0a)(x)"), r"\textcolor{#f0a}{x}");
        assert_eq!(to_latex("color(red)(x)"), r"\textcolor{red}{x}");
        let root = to_latex("root(]a)(x)");
        assert!(root.contains(r"\sqrt[{"), "{root}");
        assert!(root.contains("}]{"), "{root}");
    }
}
