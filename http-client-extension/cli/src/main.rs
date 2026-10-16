use std::process::ExitCode;

fn main() -> ExitCode {
    zed_http::cli::run_main(std::env::args().skip(1).collect())
}
