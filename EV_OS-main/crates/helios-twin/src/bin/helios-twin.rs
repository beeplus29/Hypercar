//! `helios-twin` CLI: run scenarios, write CSV traces and a JSON summary.

use clap::{Parser, Subcommand, ValueEnum};
use helios_core::PowertrainLayout;
use helios_twin::scenarios;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Layout {
    Quad,
    DualRear,
    DualAxle,
}

impl From<Layout> for PowertrainLayout {
    fn from(l: Layout) -> Self {
        match l {
            Layout::Quad => PowertrainLayout::Quad,
            Layout::DualRear => PowertrainLayout::DualRear,
            Layout::DualAxle => PowertrainLayout::DualAxle,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "helios-twin", about = "HELIOS closed-loop digital twin")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// List scenarios.
    List,
    /// Run one scenario.
    Run {
        scenario: String,
        #[arg(long, value_enum, default_value = "quad")]
        layout: Layout,
        #[arg(long, default_value = "out")]
        out: PathBuf,
    },
    /// Run every scenario (quad) plus the launch comparison for all layouts.
    All {
        #[arg(long, default_value = "out")]
        out: PathBuf,
    },
}

fn run_one(name: &str, f: scenarios::ScenarioFn, layout: PowertrainLayout, out: &std::path::Path) -> anyhow::Result<scenarios::ScenarioResult> {
    let r = f(layout)?;
    if let Some(sim) = &r.sim {
        sim.write_csv(&out.join(format!("{name}_{layout:?}.csv").to_lowercase()))?;
    }
    println!(
        "{:<28} {:<9} {:>6} sim-s in {:>6.2} s ({:>5.0}x RT)  {}",
        r.name,
        r.layout,
        format!("{:.1}", r.sim_seconds),
        r.wall_seconds,
        r.sim_seconds / r.wall_seconds.max(1e-9),
        if r.pass() { "PASS" } else { "FAIL" }
    );
    for c in &r.checks {
        println!("    [{}] {:<42} {}", if c.pass { "x" } else { " " }, c.name, c.detail);
    }
    Ok(r)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::List => {
            for (n, _) in scenarios::ALL {
                println!("{n}");
            }
        }
        Cmd::Run { scenario, layout, out } => {
            std::fs::create_dir_all(&out)?;
            let (n, f) = scenarios::ALL
                .iter()
                .find(|(n, _)| *n == scenario)
                .ok_or_else(|| anyhow::anyhow!("unknown scenario {scenario}"))?;
            let r = run_one(n, *f, layout.into(), &out)?;
            std::fs::write(out.join(format!("{n}.json")), serde_json::to_string_pretty(&r)?)?;
            if !r.pass() {
                std::process::exit(1);
            }
        }
        Cmd::All { out } => {
            std::fs::create_dir_all(&out)?;
            let mut results = vec![];
            for (n, f) in scenarios::ALL {
                results.push(run_one(n, *f, PowertrainLayout::Quad, &out)?);
            }
            for l in [PowertrainLayout::DualRear, PowertrainLayout::DualAxle] {
                results.push(run_one("launch", scenarios::launch, l, &out)?);
                results.push(run_one("motor_loss", scenarios::fault_motor_loss, l, &out)?);
            }
            let fails = results.iter().filter(|r| !r.pass()).count();
            std::fs::write(out.join("summary.json"), serde_json::to_string_pretty(&results)?)?;
            println!("\n{} scenarios, {} failed", results.len(), fails);
            if fails > 0 {
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
