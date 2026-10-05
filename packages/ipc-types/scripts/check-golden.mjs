// Type checks the golden JSON files of rimstudio-ipc-types against the generated TypeScript types, so a
// DTO and the JSON it produces can never disagree. Run with `pnpm --filter rimstudio-ipc-types bindings:check`.
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const here = dirname(fileURLToPath(import.meta.url));
const pkg = resolve(here, "..");
const srcDir = join(pkg, "src");
const goldenDir = resolve(pkg, "../../crates/rimstudio-ipc-types/tests/golden");

// Golden file name (without .json) to the TypeScript type its content has. Every golden file needs a row.
const TYPES = {
  "api-error": "ApiError",
  "asset-info": "DesignerAssetInfoResponse",
  "ce-suggestion": "CeSuggestionDto",
  "clone-diff": "DesignerCloneDiffResponse",
  "convert-scan": "ConvertScanDto",
  "design-spec-assets": "DesignSpecDto",
  "design-spec-carried": "DesignSpecDto",
  "design-spec-melee-ce": "DesignSpecDto",
  "design-spec-ranged": "DesignSpecDto",
  "fit-report": "FitReportDto",
  "fix-apply": "ProjectLayoutFixApplyDto",
  "fix-history": "ProjectLayoutFixHistoryDto",
  "fix-plan": "ProjectLayoutFixPlanDto",
  "fix-undo": "ProjectLayoutFixUndoDto",
  "job-finished-scan": "JobResultEnvelope<LibraryScanResult>",
  "lint-files": "DesignerLintFilesResult",
  preview: "PreviewDto",
  "project-file": "ProjectFileDto",
  "project-layout-check": "ProjectLayoutCheckDto",
  "project-tree": "ProjectTreeDto",
  "write-plan": "WritePlanDto",
  "write-plan-assets": "WritePlanDto",
  "archetype-catalog": "ArchetypeCatalogDto",
  "archetype-choice": "ArchetypeChoiceDto",
  "archetype-proposal": "ArchetypeProposalDto",
  "archetype-propose-request": "DesignerArchetypeProposeRequest",
  "ce-ammo-catalog": "CeAmmoCatalogDto",
  "ce-ammo-catalog-request": "DesignerCeAmmoCatalogRequest",
  "ce-ammo-custom": "CustomAmmoDto",
  "ce-ammo-suggest-request": "DesignerCeAmmoSuggestRequest",
  "ce-ammo-suggestion": "CeAmmoSuggestionDto",
  "link-requests":
    "{ status: ProjectLinkStatusRequest; create: ProjectLinkCreateRequest; createDefaults: ProjectLinkCreateRequest; remove: ProjectLinkRemoveRequest }",
  "link-result-done": "ProjectLinkResultDto",
  "link-result-refused": "ProjectLinkResultDto",
  "link-status-linked": "ProjectLinkStatusDto",
};

const files = readdirSync(goldenDir)
  .filter((f) => f.endsWith(".json"))
  .sort();
const unmapped = files.filter((f) => !(f.slice(0, -5) in TYPES));
if (unmapped.length > 0) {
  console.error(`golden files without a type in scripts/check-golden.mjs: ${unmapped.join(", ")}`);
  process.exit(1);
}

const virtualName = join(srcDir, "__golden_check__.ts");
// The type expressions may be generic or inline object types: import every identifier that bindings.ts exports.
const exported = new Set(
  [...readFileSync(join(srcDir, "bindings.ts"), "utf8").matchAll(/export (?:type|interface) (\w+)/g)].map((m) => m[1]),
);
const names = [
  ...new Set(Object.values(TYPES).flatMap((t) => (t.match(/[A-Za-z_][A-Za-z0-9_]*/g) ?? []).filter((n) => exported.has(n)))),
];
let source = `import type { ${names.join(", ")} } from "./bindings";\n`;
files.forEach((file, i) => {
  const type = TYPES[file.slice(0, -5)];
  const json = readFileSync(join(goldenDir, file), "utf8");
  source += `export const golden${i}: ${type} = ${json.trim()};\n`;
});

const configPath = join(pkg, "tsconfig.json");
const config = ts.readConfigFile(configPath, ts.sys.readFile);
const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, pkg);
const options = { ...parsed.options, noEmit: true, noUnusedLocals: false };
const host = ts.createCompilerHost(options);
const realRead = host.getSourceFile.bind(host);
host.getSourceFile = (name, lang, ...rest) =>
  name === virtualName ? ts.createSourceFile(name, source, lang) : realRead(name, lang, ...rest);
const realExists = host.fileExists.bind(host);
host.fileExists = (name) => name === virtualName || realExists(name);
const realReadFile = host.readFile.bind(host);
host.readFile = (name) => (name === virtualName ? source : realReadFile(name));

const program = ts.createProgram([join(srcDir, "bindings.ts"), virtualName], options, host);
const diagnostics = ts.getPreEmitDiagnostics(program);
if (diagnostics.length > 0) {
  const lines = source.split("\n");
  for (const d of diagnostics) {
    const text = ts.flattenDiagnosticMessageText(d.messageText, "\n");
    if (d.file && d.file.fileName === virtualName && d.start !== undefined) {
      const { line } = d.file.getLineAndCharacterOfPosition(d.start);
      let index = line;
      while (index > 0 && !lines[index]?.startsWith("export const golden")) index -= 1;
      const owner = Number(/golden(\d+)/.exec(lines[index] ?? "")?.[1] ?? -1);
      console.error(`${files[owner] ?? "?"}: ${text}`);
    } else {
      console.error(text);
    }
  }
  process.exit(1);
}
process.stdout.write(`golden types ok: ${files.length} files\n`);
