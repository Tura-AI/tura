#!/usr/bin/env node
import { main } from "./cli.js";

main(process.argv.slice(2)).catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`tura: ${message}`);
  process.exitCode =
    typeof error === "object" && error && "exitCode" in error
      ? Number((error as { exitCode: number }).exitCode)
      : 1;
});
