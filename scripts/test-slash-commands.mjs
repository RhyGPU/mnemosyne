import assert from "node:assert/strict";
import { readFileSync, writeFileSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import ts from "typescript";

const source = readFileSync("src/slashCommands.ts", "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ES2020,
    target: ts.ScriptTarget.ES2020,
  },
}).outputText;
const tempPath = join(tmpdir(), `mnemosyne-slash-commands-${process.pid}.mjs`);
writeFileSync(tempPath, compiled);

try {
  const mod = await import(pathToFileURL(tempPath).href);

  assert.equal(mod.shouldOpenSlashMenu("/"), true, "slash_menu_opens_on_slash");
  assert.deepEqual(
    mod.filteredSlashCommands("/o").map((item) => item.command),
    ["/ooc"],
    "slash_menu_filters_commands",
  );
  assert.equal(
    mod.completeSlashCommandInput("/o", "/ooc"),
    "/ooc ",
    "tab_autocompletes_selected_command",
  );
  assert.deepEqual(
    mod.filteredSlashCommands("/p").map((item) => item.command),
    ["/persona"],
    "slash_menu_includes_persona_command",
  );
  assert.deepEqual(
    mod.filteredSlashCommands("/sta").map((item) => item.command),
    ["/state"],
    "slash_menu_excludes_deprecated_status_alias",
  );
  assert.equal(mod.nextSlashCommandIndex(0, 3, 1), 1, "arrow_keys_change_selection_down");
  assert.equal(mod.nextSlashCommandIndex(0, 3, -1), 2, "arrow_keys_change_selection_up");
  assert.equal(
    mod.canEnterSelectSlashSuggestion("/ask"),
    true,
    "enter_selects_suggestion_when_menu_open",
  );
  assert.equal(
    mod.canEnterSelectSlashSuggestion("/ask plan"),
    false,
    "enter_submits_when_body_text_exists",
  );
  assert.equal(
    mod.shouldOpenSlashMenu("I type /o"),
    false,
    "slash_in_middle_of_text_does_not_open_menu",
  );
} finally {
  unlinkSync(tempPath);
}
