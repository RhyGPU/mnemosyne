import assert from "node:assert/strict";
import fs from "node:fs/promises";
import ts from "typescript";

async function importTypeScriptModule(path) {
  const source = await fs.readFile(path, "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: path,
  });
  return import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);
}

const messageLifecycle = await importTypeScriptModule(
  "src/features/chat/model/messageLifecycle.ts",
);
const benchmarkRuntime = await importTypeScriptModule(
  "src/features/benchmark/model/benchmarkRuntime.ts",
);
const devCommands = await importTypeScriptModule("src/features/dev/commands.ts");
const assistantDisplay = await importTypeScriptModule(
  "src/features/chat/model/assistantDisplay.ts",
);
const stateMapPresentation = await importTypeScriptModule(
  "src/features/state-map/model/stateMapPresentation.ts",
);

const savedUser = {
  id: 1,
  conversation_id: "conversation-1",
  role: "user",
  content: "Hello",
  created_at: 1,
};
const savedAssistant = {
  id: 2,
  conversation_id: "conversation-1",
  role: "assistant",
  content: "Hi.",
  created_at: 2,
};
const pendingAssistant = {
  ...savedAssistant,
  id: -2,
  pending: true,
};

{
  const rendered = messageLifecycle.prepareMessagesForRender([
    savedUser,
    savedAssistant,
    { ...savedAssistant },
    pendingAssistant,
  ]);
  assert.equal(rendered.messages.length, 2);
  assert.equal(rendered.trace.duplicate_saved_suppressed, 1);
  assert.equal(rendered.trace.pending_replaced_by_saved, 1);
}

{
  const upserted = messageLifecycle.upsertSavedChatMessage(
    [savedUser, pendingAssistant],
    savedAssistant,
  );
  assert.deepEqual(
    upserted.messages.map((message) => message.id),
    [1, 2],
  );
  assert.equal(upserted.trace.pending_assistant_replaced_by_saved, 1);
}

{
  const seeded = messageLifecycle.seedStreamingTurn([], "conversation-1", "Hello");
  const streamed = messageLifecycle.appendStreamingChunk(
    seeded,
    "conversation-1",
    "partial",
  );
  assert.equal(streamed.length, 2);
  assert.equal(streamed.at(-1).content, "partial");
  assert.equal(streamed.at(-1).pending, true);
}

{
  const faithful = benchmarkRuntime.benchmarkLiveUpdaterOverride({
    strict_tool_evaluator: false,
    wait_for_evaluator_each_turn: false,
  });
  assert.deepEqual(faithful, {});

  const sequenced = benchmarkRuntime.benchmarkLiveUpdaterOverride({
    strict_tool_evaluator: false,
    wait_for_evaluator_each_turn: true,
  });
  assert.equal(sequenced.evaluator_background_enabled, true);
  assert.equal(sequenced.wait_for_evaluator_before_next_turn, false);
}

{
  assert.equal(
    benchmarkRuntime.turnResultEvaluatorCompletedOrSkipped({
      debug: { state_updater_status: "completed" },
    }),
    true,
  );
  assert.equal(
    benchmarkRuntime.benchmarkEvaluatorJobCompletedOrSkipped({ status: "failed" }),
    false,
  );
}

{
  assert.deepEqual(devCommands.parseDevCommandArgs('{"turn_count":"7"}'), {
    turn_count: "7",
  });
  assert.equal(devCommands.devNumberArg({ turn_count: "7" }, "turn_count", 1), 7);
  assert.equal(devCommands.devBooleanArg({ strict: "false" }, "strict", true), false);
  assert.throws(() => devCommands.parseDevCommandArgs("[]"), /must be an object/);
}

{
  const display = assistantDisplay.splitAssistantDisplay(
    `Aurora waits.

\`\`\`status
Scene | Focus: Aurora | Atmosphere: Rain
\`\`\``,
  );
  assert.equal(display.prose, "Aurora waits.");
  assert.equal(
    display.status,
    "Scene | Focus: Aurora | Atmosphere: Rain",
  );
  assert.equal(display.prose.includes("```status"), false);

  const legacyDisplay = assistantDisplay.splitAssistantDisplay(
    `Visible prose.
[HIDDEN STATE]
secret patch`,
  );
  assert.equal(legacyDisplay.prose, "Visible prose.");
  assert.equal(legacyDisplay.status, null);
}

{
  const presentation = stateMapPresentation.buildStateMapPresentation({
    sessions: [],
    scenes: [],
    timeline: [],
    memories: [],
    characters: [
      { session_id: "new", session_title: "Latest", name: "Aurora", role: "session_clone", detail: "new" },
      { session_id: "old", session_title: "Older", name: "Aurora", role: "session_clone", detail: "old" },
      { session_id: "new", session_title: "Latest", name: "user", role: "relationship", detail: "player" },
    ],
    relationships: [
      { session_id: "new", session_title: "Latest", soul_name: "Aurora", target: "preset_male", love_type: "relationship", trust: 1, affection: 2, intimacy: 3, fear: 4, desire: 5 },
      { session_id: "old", session_title: "Older", soul_name: "Aurora", target: "user", love_type: "relationship", trust: 9, affection: 9, intimacy: 9, fear: 9, desire: 9 },
    ],
    objects: [
      { session_id: "new", session_title: "Latest", name: "Phone", kind: "key_object", owner: "unknown", location: "", status: "tracked", summary: "", confidence: 1 },
      { session_id: "old", session_title: "Older", name: "Phone", kind: "key_object", owner: "unknown", location: "", status: "tracked", summary: "", confidence: 1 },
    ],
  });
  assert.deepEqual(presentation.characters.map((item) => item.name), ["Aurora", "You"]);
  assert.equal(presentation.characters[0].role, "Soul");
  assert.equal(presentation.characters[0].session_count, 2);
  assert.equal(presentation.relationships.length, 1);
  assert.equal(presentation.relationships[0].target, "You");
  assert.equal(presentation.relationships[0].trust, 1);
  assert.equal(presentation.objects.length, 1);
  assert.equal(presentation.objects[0].owner, "Unassigned");
  assert.equal(
    stateMapPresentation.humanizeStateMapText("Aurora and preset_male"),
    "Aurora and You",
  );
}

console.log("frontend model characterization tests passed");
