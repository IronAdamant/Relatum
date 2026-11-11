fn main() {
    let mut args = std::env::args();
    let argv0 = args.next().unwrap_or_default();
    let rest: Vec<String> = args.collect();
    // `--version` alone, for every binary; not in the argparse-shaped help.
    if matches!(rest.as_slice(), [flag] if flag == "--version" || flag == "-V") {
        let name = relatum::prog::stem(&argv0);
        let name = if name.ends_with("-mcp") { name } else { relatum::prog::prog_name(&argv0) };
        println!("{name} {}", relatum::prog::VERSION);
        std::process::exit(0);
    }
    let code = if relatum::prog::stem(&argv0).ends_with("-mcp") {
        relatum::mcp::main_mcp(&rest)
    } else {
        let prog = relatum::prog::prog_name(&argv0);
        relatum::cli::main_from_env(&rest, &prog)
    };
    std::process::exit(code);
}
