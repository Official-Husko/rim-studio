# schema-probe

Feasibility experiment for a dev-time generator that derives a compact XML schema from the game assembly by reflection.

1. `Program.cs` + `probe.csproj`: reads Assembly-CSharp.dll through MetadataLoadContext (no game code runs) and writes `schema.json` in the current directory. Argument 1 is the game `Managed` folder.
2. `validate_vanilla.py <game Data folder>`: validates vanilla Core and DLC Defs against that schema. Set `RIMSTUDIO_SCHEMA` to the schema path (default: next to the script). Run with PYTHONDONTWRITEBYTECODE=1.

The generated schema (about 3 MB, 296 KB gzipped) is not committed. Results are in docs/research/modding-toolkit-scope.md.
