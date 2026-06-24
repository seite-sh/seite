use clap::{Args, Subcommand};

#[derive(Args)]
pub struct TelemetryArgs {
    #[command(subcommand)]
    pub command: TelemetryCommand,
}

#[derive(Subcommand)]
pub enum TelemetryCommand {
    /// Show whether telemetry is enabled and why
    Status,
    /// Enable anonymous usage telemetry
    On,
    /// Disable anonymous usage telemetry
    Off,
}

pub fn run(args: &TelemetryArgs) -> anyhow::Result<()> {
    match args.command {
        TelemetryCommand::Status => {
            println!("{}", crate::telemetry::status_line());
        }
        TelemetryCommand::On => match crate::telemetry::set_enabled(true) {
            Some(()) => println!("Telemetry enabled."),
            None => eprintln!(
                "Could not save telemetry preference (could not write ~/.seite/telemetry.json)."
            ),
        },
        TelemetryCommand::Off => match crate::telemetry::set_enabled(false) {
            Some(()) => println!("Telemetry disabled."),
            None => eprintln!(
                "Could not save telemetry preference (could not write ~/.seite/telemetry.json)."
            ),
        },
    }
    Ok(())
}
