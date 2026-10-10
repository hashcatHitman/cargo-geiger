//! The outer CLI parts of the `cargo-geiger` cargo plugin executable.
//! TODO: Refactor this file to only deal with command line argument processing.

extern crate cargo;
extern crate colored;
extern crate petgraph;
extern crate strum;
extern crate strum_macros;

use cargo_geiger::args::{Args, HELP};
use cargo_geiger::cli::{get_cargo_metadata, get_krates, get_workspace};
use cargo_geiger::graph::build_graph;
use cargo_geiger::mapping::{CargoMetadataParameters, QueryResolve};
use cargo_geiger::readme::create_or_replace_section_in_readme;
use cargo_geiger::scan::{scan, FoundWarningsError, ScanResult};

use cargo::core::shell::Shell;
use cargo::util::important_paths;
use cargo::{CliError, CliResult, GlobalContext};

const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");

fn cli_result_main(args: &Args) -> CliResult {
    if args.version {
        println!("cargo-geiger {}", VERSION.unwrap_or("unknown version"));
        return Ok(());
    }
    if args.help {
        println!("{}", HELP);
        return Ok(());
    }

    let mut gctx: cargo::GlobalContext = GlobalContext::default()?;
    args.update_config(&mut gctx)?;

    #[expect(clippy::undeclared_type, reason = "Borked!")]
    let cargo_metadata = get_cargo_metadata(args, &gctx)?;
    let krates: krates::Krates = get_krates(&cargo_metadata)?;

    let cargo_metadata_parameters: cargo_geiger::mapping::CargoMetadataParameters<'_> = CargoMetadataParameters {
        metadata: &cargo_metadata,
        krates: &krates,
    };

    let workspace: cargo::core::Workspace<'_> =
        get_workspace(&gctx, args.manifest_path.clone())?;

    #[expect(clippy::undeclared_type, reason = "Borked!")]
    let cargo_metadata_root_package_id = if let Some(
        cargo_metadata_root_package,
    ) = cargo_metadata.root_package()
    {
        cargo_metadata_root_package.id.clone()
    } else {
        eprintln!(
            "manifest path `{}` is a virtual manifest, but this command requires running against an actual package in this workspace",
            match args.manifest_path.clone() {
                Some(path) => path,
                None => important_paths::find_root_manifest_for_wd(gctx.cwd())?,
            }.as_os_str().to_str().unwrap()
        );

        return Err(CliError::code(1));
    };

    let global_rustc: cargo::util::Rustc =
        gctx.load_global_rustc(Some(&workspace))?;

    let graph: cargo_geiger::graph::Graph = build_graph(
        args,
        &cargo_metadata_parameters,
        &global_rustc.host,
        &global_rustc.path,
        cargo_metadata_root_package_id.clone(),
    )?;

    #[expect(clippy::undeclared_type, reason = "Borked!")]
    let query_resolve_root_package_id = args.package.as_ref().map_or(
        cargo_metadata_root_package_id.clone(),
        |package_query| {
            krates
                .query_resolve(package_query)
                .unwrap_or(cargo_metadata_root_package_id)
        },
    );

    let ScanResult {
        scan_output_lines,
        warning_count,
    }: cargo_geiger::scan::ScanResult = scan(
        args,
        &cargo_metadata_parameters,
        &gctx,
        &graph,
        query_resolve_root_package_id,
        &workspace,
    )?;

    if args.readme_args.update_readme {
        create_or_replace_section_in_readme(
            &args.readme_args,
            &scan_output_lines,
        )?;
    } else {
        for scan_output_line in scan_output_lines {
            println!("{}", scan_output_line);
        }
    }

    if warning_count > 0 {
        return Err(CliError::new(
            anyhow::Error::new(FoundWarningsError { warning_count }),
            1,
        ));
    }

    Ok(())
}

fn main() {
    let args: cargo_geiger::args::Args =
        Args::parse_args(pico_args::Arguments::from_env()).unwrap();
    if let Err(e) = cli_result_main(&args) {
        let mut shell: cargo::core::Shell = Shell::new();
        cargo::exit_with_error(e, &mut shell)
    }
}
