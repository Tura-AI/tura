import { CliUsageError } from "../types/common.js";
import { t } from "../i18n.js";

const COMMANDS = [
  "run",
  "exec",
  "bash",
  "zsh",
  "shll",
  "resume",
  "session",
  "config",
  "provider",
  "agent",
  "persona",
  "project",
  "file",
  "command",
  "inspect",
  "gateway",
  "completion",
];

export function completionCommand(args: string[]): void {
  const shell = args[0] ?? "bash";
  if (shell === "bash") {
    process.stdout.write(
      `_tura_complete(){ COMPREPLY=( $(compgen -W "${COMMANDS.join(" ")}" -- "\${COMP_WORDS[COMP_CWORD]}") ); }\ncomplete -F _tura_complete tura\n`,
    );
    return;
  }
  if (shell === "zsh") {
    process.stdout.write(`#compdef tura\n_arguments '1:command:(${COMMANDS.join(" ")})'\n`);
    return;
  }
  if (shell === "fish") {
    for (const command of COMMANDS) process.stdout.write(`complete -c tura -f -a ${command}\n`);
    return;
  }
  throw new CliUsageError(t("shellUnsupported", { shell }));
}
