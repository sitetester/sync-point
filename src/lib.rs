// It automatically acts as the entry point for declaring and organizing modules.
// This eliminates the need to manually declare `mod api;` in `main.rs`.
// Instead, `lib.rs` defines all of  project's modules, which can be accessed
// by any binary (`main.rs`) or test module in the project.

pub mod api; // Declare the api module
