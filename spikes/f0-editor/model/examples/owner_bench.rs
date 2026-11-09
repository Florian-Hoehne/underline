//! Native timing of the owner alone (no IPC, no DOM): what one input costs
//! in the Rust code that both variants run.
//!
//! `cargo run --release -p f0-model --example owner_bench [--features real-facade]`

use std::time::Instant;

use f0_model::{FacadeInput, Input, Owner, SourceInput};

fn pct(mut v: Vec<f64>) -> String {
    v.sort_by(f64::total_cmp);
    let q = |p: f64| {
        v[((p * v.len() as f64).ceil() as usize)
            .saturating_sub(1)
            .min(v.len() - 1)]
    };
    format!(
        "median {:.3} ms, p99 {:.3} ms, max {:.3} ms",
        q(0.5),
        q(0.99),
        v[v.len() - 1]
    )
}

fn main() {
    let note = f0_model::note::generate();
    let mut owner = Owner::new(&note, false);
    let t = Instant::now();
    let html = owner.render_all();
    println!(
        "parse + render all: {:.1} ms, {} blocks, {} bytes of HTML",
        t.elapsed().as_secs_f64() * 1e3,
        owner.block_count(),
        html.len()
    );
    let t = Instant::now();
    let blocks = f0_model::parse(&note);
    println!(
        "parse only: {:.1} ms ({} blocks)",
        t.elapsed().as_secs_f64() * 1e3,
        blocks.len()
    );
    for pos in owner.bench_positions().into_iter().rev() {
        let mut facade = Vec::new();
        for i in 0..300 {
            let t = Instant::now();
            owner.facade_input(&FacadeInput {
                block: pos.block,
                at: pos.at + i,
                input: Input::InsertText {
                    text: "e".to_owned(),
                },
            });
            facade.push(t.elapsed().as_secs_f64() * 1e3);
        }
        let mut source = Vec::new();
        for i in 0..300 {
            let t = Instant::now();
            owner.source_input(&SourceInput {
                at: pos.source + i,
                input: Input::InsertText {
                    text: "e".to_owned(),
                },
            });
            source.push(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("{} facade type: {}", pos.name, pct(facade));
        println!("{} source type: {}", pos.name, pct(source));
        let mut back = Vec::new();
        for i in 0..50 {
            let t = Instant::now();
            owner.facade_input(&FacadeInput {
                block: pos.block,
                at: pos.at + 300 - i,
                input: Input::DeleteBackward,
            });
            back.push(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("{} facade backspace: {}", pos.name, pct(back));
    }
}
