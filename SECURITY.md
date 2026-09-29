# Security policy

## Supported versions

The project is experimental and pre-1.0. Security fixes target the current development branch (`0.1.0`); there is no supported stable binary. Historical `v1.0.0` artifacts are not recommended for use and do not receive maintenance updates.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability or include credentials, private source, repository paths, or provider output in a public report.

Use GitHub private vulnerability reporting from the repository's **Security** tab. Include:

- the affected version or commit;
- the operation mode and provider involved;
- the expected and observed custody boundary;
- minimal reproduction steps using synthetic data;
- impact and any known workaround.

Reports will be reviewed privately, and reporters will be credited unless anonymity is requested. Enabling GitHub private vulnerability reporting is a publication gate in [docs/PUBLIC_RELEASE_CHECKLIST.md](docs/PUBLIC_RELEASE_CHECKLIST.md).

## Security posture

The Staff Room treats repository mutation, process execution, local voice assets, and provider authentication as separate trust boundaries. The current guarantees and their enforcing symbols are documented in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Autonomous Ship is not accepted; its unmet controls remain public in [docs/AUTONOMOUS_ACCEPTANCE_CONTRACT.md](docs/AUTONOMOUS_ACCEPTANCE_CONTRACT.md).
