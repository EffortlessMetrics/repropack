use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use repropack_capture::{capture, CaptureOptions, PacketFormat};
use repropack_git::BundleMode;
use repropack_model::ReplayPolicy;
use repropack_pack::{materialize, unpack_rpk};
use repropack_replay::{replay, ReplayOptions};
use repropack_render::render_manifest_markdown;

#[derive(Parser)]
#[command(name = "repropack", version, about = "Commit-aware failure packet generator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Capture(CaptureArgs),
    Inspect(InspectArgs),
    Replay(ReplayArgs),
    Unpack(UnpackArgs),
    Emit(EmitArgs),
}

#[derive(Args)]
struct CaptureArgs {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    base: Option<String>,
    #[arg(long)]
    head: Option<String>,
    #[arg(long = "include")]
    include_globs: Vec<String>,
    #[arg(long = "output")]
    output_globs: Vec<String>,
    #[arg(long = "env-allow")]
    env_allow: Vec<String>,
    #[arg(long = "env-deny")]
    env_deny: Vec<String>,
    #[arg(long = "git-bundle", value_enum, default_value = "auto")]
    git_bundle: GitBundleArg,
    #[arg(long, value_enum, default_value = "rpk")]
    format: FormatArg,
    #[arg(short = 'o', long)]
    out: Option<PathBuf>,
    #[arg(long = "policy", value_enum, default_value = "safe")]
    policy: ReplayPolicyArg,
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

#[derive(Args)]
struct InspectArgs {
    packet: PathBuf,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    tree: bool,
}

#[derive(Args)]
struct ReplayArgs {
    packet: PathBuf,
    #[arg(long)]
    into: Option<PathBuf>,
    #[arg(long = "set-env")]
    set_env: Vec<String>,
    #[arg(long)]
    no_run: bool,
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct UnpackArgs {
    packet: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct EmitArgs {
    #[command(subcommand)]
    target: EmitTarget,
}

#[derive(Subcommand)]
enum EmitTarget {
    GithubActions(EmitGithubActionsArgs),
}

#[derive(Args)]
struct EmitGithubActionsArgs {
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FormatArg {
    Rpk,
    Dir,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum GitBundleArg {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ReplayPolicyArg {
    Safe,
    Confirm,
    Disabled,
}

fn main() {
    let exit_code = match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            2
        }
    };

    std::process::exit(exit_code);
}

fn run() -> Result<i32> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Capture(args) => {
            let options = CaptureOptions {
                name: args.name,
                base: args.base,
                head: args.head,
                include_globs: args.include_globs,
                output_globs: args.output_globs,
                env_allow: if args.env_allow.is_empty() {
                    CaptureOptions::default().env_allow
                } else {
                    args.env_allow
                },
                env_deny: if args.env_deny.is_empty() {
                    CaptureOptions::default().env_deny
                } else {
                    args.env_deny
                },
                bundle_mode: match args.git_bundle {
                    GitBundleArg::Auto => BundleMode::Auto,
                    GitBundleArg::Always => BundleMode::Always,
                    GitBundleArg::Never => BundleMode::Never,
                },
                format: match args.format {
                    FormatArg::Rpk => PacketFormat::Rpk,
                    FormatArg::Dir => PacketFormat::Directory,
                },
                output_path: args.out,
                replay_policy: match args.policy {
                    ReplayPolicyArg::Safe => ReplayPolicy::Safe,
                    ReplayPolicyArg::Confirm => ReplayPolicy::Confirm,
                    ReplayPolicyArg::Disabled => ReplayPolicy::Disabled,
                },
            };

            let result = capture(&args.command, &options)?;
            println!("{}", result.packet_path.display());
            Ok(result.command_exit_code)
        }
        Commands::Inspect(args) => {
            let materialized = materialize(&args.packet)
                .with_context(|| format!("materializing {}", args.packet.display()))?;
            let manifest = repropack_model::PacketManifest::read_from_path(&materialized.manifest_path())
                .context("reading manifest")?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&manifest)?);
            } else if args.tree {
                print_tree(&materialized.root)?;
            } else {
                let summary_path = materialized.root.join("summary.md");
                if summary_path.exists() {
                    print!("{}", fs::read_to_string(&summary_path)?);
                } else {
                    print!("{}", render_manifest_markdown(&manifest));
                }
            }

            Ok(0)
        }
        Commands::Replay(args) => {
            let set_env = parse_set_env(&args.set_env)?;
            let result = replay(
                &args.packet,
                &ReplayOptions {
                    into: args.into,
                    set_env,
                    no_run: args.no_run,
                    force: args.force,
                },
            )?;

            println!("{}", result.receipt_path.display());
            Ok(result.command_exit_code)
        }
        Commands::Unpack(args) => {
            unpack_rpk(&args.packet, &args.out)?;
            println!("{}", args.out.display());
            Ok(0)
        }
        Commands::Emit(args) => match args.target {
            EmitTarget::GithubActions(target) => {
                let yaml = github_actions_snippet();
                if let Some(path) = target.out {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)
                            .with_context(|| format!("creating {}", parent.display()))?;
                    }
                    fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))?;
                    println!("{}", path.display());
                } else {
                    print!("{yaml}");
                }
                Ok(0)
            }
        },
    }
}

fn parse_set_env(entries: &[String]) -> Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();

    for entry in entries {
        let Some((key, value)) = entry.split_once('=') else {
            return Err(anyhow::anyhow!("expected KEY=VALUE for --set-env, got `{entry}`"));
        };
        map.insert(key.to_string(), value.to_string());
    }

    Ok(map)
}

fn print_tree(root: &Path) -> Result<()> {
    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(root).into_iter().filter_map(|entry| entry.ok()) {
        let relative = entry.path().strip_prefix(root).unwrap();
        if relative.as_os_str().is_empty() {
            continue;
        }
        let suffix = if entry.file_type().is_dir() { "/" } else { "" };
        paths.push(format!("{}{}", normalize_relative(relative), suffix));
    }

    paths.sort();
    for path in paths {
        println!("{path}");
    }

    Ok(())
}

fn normalize_relative(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            std::path::Component::CurDir => Some(".".to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn github_actions_snippet() -> &'static str {
    r#"name: repropack-capture

on:
  workflow_dispatch:

jobs:
  capture:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: dtolnay/rust-toolchain@stable

      - name: Build repropack
        run: cargo build --release -p repropack-cli

      - name: Capture packet
        run: |
          ./target/release/repropack capture \
            --name ci-red \
            --git-bundle auto \
            --format rpk \
            -- cargo test
        continue-on-error: true

      - name: Upload packet
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: repropack-${{ github.run_id }}
          path: "*.rpk"
"#
}
