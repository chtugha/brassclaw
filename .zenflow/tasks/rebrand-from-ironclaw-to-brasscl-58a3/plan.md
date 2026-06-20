# Rebranding Plan: IronClaw to BrassClaw

### [x] Step: Rebrand local codebase
- Case-preserving replacements of "ironclaw" with "brassclaw" in all file contents
- Git-rename all files and directories containing "ironclaw" to "brassclaw"
- Run build checks to ensure codebase compiles clean locally

### [x] Step: Rename repository references
- Update repository references to chtugha/brassclaw in README, CONTRIBUTING, script files
- Update remote URL in .git/config to chtugha/brassclaw

### [x] Step: Deploy and test on remote machine
- [x] Establish SSH connection to remote machine 192.168.10.169
- [x] Clean up any existing ironclaw instances, files, and directories
- [x] Run install.sh to install brassclaw
- [x] Test installation via browser-ui chat
