#!/usr/bin/env node
import { main } from "./cli.js";
import { userFacingError } from "./gateway/errors.js";

main(process.argv.slice(2)).catch((error) => {
  const message = userFacingError(error);
  console.error(`tura: ${message}`);
  process.exitCode =
    typeof error === "object" && error && "exitCode" in error
      ? Number((error as { exitCode: number }).exitCode)
      : 1;
});
