# Linear Agent

A CLI tool that fetches tickets from Linear, enriches them with detailed information, and uses Claude to generate implementation plans.

## Features

- Interactive CLI interface
- Fetches tickets assigned to a specific user from Linear
- Fetch individual tickets directly by their ID
- Enriches tickets with labels, comments, parent/child relationships, and related tickets
- Uses Claude to generate detailed implementation plans
- Stores ticket information and implementation plans as Markdown files in separate directories
- Process previously saved tickets from file without needing Linear API access
- Configurable through command-line arguments or environment variables (.env file)
- Cross-platform support for Linux, macOS, and Windows

## Installation

### Prerequisites

- Linear API key
- Anthropic API key

### Download pre-built binaries

You can download pre-built binaries for your platform from the [GitHub Releases](https://github.com/yourusername/linear-agent/releases) page.

Available binaries:
- Linux (x86_64): `linear-agent-linux-amd64`
- Linux (ARM64): `linear-agent-linux-arm64`
- macOS (x86_64): `linear-agent-macos-amd64`
- macOS (Apple Silicon): `linear-agent-macos-arm64`
- Windows: `linear-agent-windows-amd64.exe`

#### One-line installation (Linux/macOS)

For Linux (x86_64):
```bash
curl -L https://github.com/yourusername/linear-agent/releases/latest/download/linear-agent-linux-amd64 -o linear-agent && chmod +x linear-agent && sudo mv linear-agent /usr/local/bin/
```

For macOS (Intel):
```bash
curl -L https://github.com/yourusername/linear-agent/releases/latest/download/linear-agent-macos-amd64 -o linear-agent && chmod +x linear-agent && sudo mv linear-agent /usr/local/bin/
```

For macOS (Apple Silicon):
```bash
curl -L https://github.com/yourusername/linear-agent/releases/latest/download/linear-agent-macos-arm64 -o linear-agent && chmod +x linear-agent && sudo mv linear-agent /usr/local/bin/
```

#### Manual installation

After downloading, make the file executable (Linux/macOS):
```bash
chmod +x linear-agent-*
```

Move the executable to a location in your PATH:
```bash
sudo mv linear-agent-* /usr/local/bin/linear-agent
```

### Updating

You can check for updates using:
```bash
linear-agent --check-update
```

This will compare your current version with the latest release and provide update instructions if a new version is available.

To update manually, you can use the same installation command that you used initially. It will download the latest release and replace your existing installation.

### Building from source

If you prefer to build from source:

1. Install Rust and Cargo ([Install Rust](https://www.rust-lang.org/tools/install))

2. Clone the repository:
   ```
   git clone https://github.com/yourusername/linear-agent.git
   cd linear-agent
   ```

3. Build the executable:
   ```
   cargo build --release
   ```

4. The compiled binary will be available at `target/release/linear-agent`

5. Optional: Move the binary to a location in your PATH:
   ```
   cp target/release/linear-agent ~/.local/bin/
   ```

## Usage

### First-time setup

Run the setup wizard to configure your API keys and preferences:

```
linear-agent --setup
```

This will guide you through setting up your Linear and Anthropic API keys, and configuring the default settings.

### Basic usage

```
# Fetch tickets and save them to the tickets/ directory
linear-agent --user "Your Name"

# Generate implementation plans along with saving tickets
linear-agent --user "Your Name" --plan

# Process a previously saved ticket file to generate a plan
linear-agent --ticket tickets/LIN-123-My_Ticket_Title.md --plan

# Fetch a specific ticket by ID and save it
linear-agent --ticket-id LIN-123
```

Basic usage will:
1. Fetch tickets assigned to you in Linear
2. Display a list of tickets
3. Let you select which tickets to analyze
4. Save the ticket information as Markdown files in the `tickets/` directory

With the `--plan` flag, it will also:
5. Generate implementation plans using Claude
6. Save the implementation plans to the `implementation_plans/` directory

You can also process a previously saved ticket file to generate an implementation plan without accessing Linear API.

### Command-line options

```
USAGE:
    linear-agent [OPTIONS]

OPTIONS:
    -e, --env <FILE>                  Path to .env file
    -u, --user <USERNAME>             Linear user to analyze tickets for
    -t, --team <TEAMNAME>             Linear team name
    -s, --states <STATES>             Comma-separated list of ticket states (e.g. 'Open,In Progress')
    -m, --model <MODEL>               Anthropic model to use
    -o, --output <DIR>                Output directory for implementation plans [default: implementation_plans]
    --tickets-dir <DIR>               Directory for saving ticket information [default: tickets]
    --ticket <FILE>                   Path to a previously saved ticket markdown file to process
    --ticket-id <ID>                  Linear ticket ID to fetch and save (e.g. 'LIN-123')
    --plan                            Generate implementation plans (default just saves ticket info)
    --verbose                         Show debug information and API responses
    --setup                           Run setup wizard to configure credentials
    --check-update                    Check if a new version is available
    -h, --help                        Print help
    -V, --version                     Print version
```

### Configuration via .env file

You can create a `.env` file with the following environment variables:

```
LINEAR_API_KEY=your-linear-api-key
ANTHROPIC_API_KEY=your-anthropic-api-key
LINEAR_TEAM_NAME=Engineering
LINEAR_AGENT_USER=Your Name
LINEAR_AGENT_STATES=Open,In Progress
ANTHROPIC_MODEL=claude-3-7-sonnet-20250219
```

The tool will look for the `.env` file in the following locations:
1. Path specified with `-e, --env`
2. `.env` in the current directory
3. `.linear-agent/.env` in your home directory

### Environment variables

You can also directly set environment variables in your shell:

- `LINEAR_API_KEY`: Your Linear API key
- `ANTHROPIC_API_KEY`: Your Anthropic API key
- `LINEAR_TEAM_NAME`: Linear team name
- `LINEAR_AGENT_USER`: Linear user name
- `LINEAR_AGENT_STATES`: Comma-separated list of ticket states
- `ANTHROPIC_MODEL`: Anthropic model to use

## Output

### Ticket Files

Ticket information is saved as Markdown files in the tickets directory (default: `tickets/`). Each file includes:

- Ticket metadata (ID, state, priority, estimate, URL, labels)
- Ticket description
- Comments
- Related tickets
- Child tickets

These files can be used as input for generating implementation plans later using the `--ticket` option.

### Implementation Plans

Implementation plans are saved as Markdown files in the output directory (default: `implementation_plans/`). Each file includes:

- Ticket metadata (ID, state, priority, estimate, URL)
- Detailed implementation plan generated by Claude:
  - Task overview
  - Technical requirements
  - Step-by-step implementation approach
  - Potential challenges and solutions
  - Testing strategy
  - Estimated effort

## License

MIT