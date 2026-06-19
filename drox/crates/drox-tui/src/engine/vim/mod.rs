//! Mode vim du composer (sous-ensemble leak `useVimInput.ts` + `src/vim/*`).

mod cursor;
mod handler;

pub use handler::{VimComposer, VimKeyResult, VimMode};
