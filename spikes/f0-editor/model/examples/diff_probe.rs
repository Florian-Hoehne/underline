//! Prints the size of the facade patches of Backspace and Enter.
use f0_model::{FacadeInput, Input, Owner};

fn main() {
    let note = f0_model::note::generate();
    let mut owner = Owner::new(&note, false);
    let pos = owner.bench_positions()[2].clone();
    for i in 0..5 {
        owner.facade_input(&FacadeInput {
            block: pos.block,
            at: pos.at + i,
            input: Input::InsertText { text: "e".into() },
        });
    }
    let before = f0_model::parse(&owner.text());
    owner.facade_input(&FacadeInput {
        block: pos.block,
        at: pos.at + 5,
        input: Input::DeleteBackward,
    });
    let after = f0_model::parse(&owner.text());
    println!("{:?}\n{:?}", before[3675], after[3675]);
    for input in [
        Input::DeleteBackward,
        Input::InsertParagraph,
        Input::DeleteBackward,
    ] {
        let p = owner.facade_input(&FacadeInput {
            block: pos.block,
            at: pos.at + 5,
            input: input.clone(),
        });
        println!(
            "{input:?}: from {} to {} html {} blocks {} cursor {}:{}",
            p.from,
            p.to,
            p.html.len(),
            p.blocks,
            p.cursor_block,
            p.cursor_at
        );
    }
}
