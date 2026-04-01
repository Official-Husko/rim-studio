package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCreateProjectCreatesExpectedLayout(t *testing.T) {
	app := NewAppWithConfigDir(t.TempDir())
	parentDir := t.TempDir()

	state, err := app.CreateProject(CreateProjectInput{
		Name:          "Steel Frontier",
		PackageID:     "pawbeans.steelfrontier",
		Author:        "Pawbeans",
		Location:      parentDir,
		TargetVersion: supportedVersion,
	})
	if err != nil {
		t.Fatalf("CreateProject returned error: %v", err)
	}

	projectPath := filepath.Join(parentDir, "Steel Frontier")
	if state.Summary.Path != projectPath {
		t.Fatalf("unexpected project path: %s", state.Summary.Path)
	}

	expectedPaths := []string{
		filepath.Join(projectPath, "About", "About.xml"),
		filepath.Join(projectPath, "Defs"),
		filepath.Join(projectPath, "Textures"),
		filepath.Join(projectPath, "Config", "rimstudio.project.json"),
	}
	for _, path := range expectedPaths {
		if _, err := os.Stat(path); err != nil {
			t.Fatalf("expected path %s to exist: %v", path, err)
		}
	}
}

func TestOpenProjectImportsExistingModAndCreatesConfig(t *testing.T) {
	app := NewAppWithConfigDir(t.TempDir())
	projectPath := filepath.Join(t.TempDir(), "ImportedMod")

	if err := os.MkdirAll(filepath.Join(projectPath, "About"), 0o755); err != nil {
		t.Fatalf("mkdir About: %v", err)
	}
	if err := os.WriteFile(filepath.Join(projectPath, "About", "About.xml"), []byte(`<?xml version="1.0" encoding="utf-8"?>
<ModMetaData>
  <name>Imported Mod</name>
  <author>Tester</author>
  <packageId>tester.imported</packageId>
</ModMetaData>
`), 0o644); err != nil {
		t.Fatalf("write About.xml: %v", err)
	}

	state, err := app.OpenProject(projectPath)
	if err != nil {
		t.Fatalf("OpenProject returned error: %v", err)
	}

	if state.Summary.Name != "Imported Mod" {
		t.Fatalf("unexpected project name: %s", state.Summary.Name)
	}

	if _, err := os.Stat(filepath.Join(projectPath, "Config", "rimstudio.project.json")); err != nil {
		t.Fatalf("expected RimStudio config to be created: %v", err)
	}
}

func TestRecentProjectsAreDedupedAndLimited(t *testing.T) {
	app := NewAppWithConfigDir(t.TempDir())
	parentDir := t.TempDir()
	names := []string{"One", "Two", "Three", "Four"}

	for _, name := range names {
		_, err := app.CreateProject(CreateProjectInput{
			Name:          name,
			PackageID:     "tester." + name,
			Author:        "Tester",
			Location:      parentDir,
			TargetVersion: supportedVersion,
		})
		if err != nil {
			t.Fatalf("CreateProject(%s) returned error: %v", name, err)
		}
	}

	bootstrap, err := app.GetAppBootstrap()
	if err != nil {
		t.Fatalf("GetAppBootstrap returned error: %v", err)
	}

	if len(bootstrap.RecentProjects) != 3 {
		t.Fatalf("expected 3 recent projects, got %d", len(bootstrap.RecentProjects))
	}
	if bootstrap.RecentProjects[0].Name != "Four" || bootstrap.RecentProjects[2].Name != "Two" {
		t.Fatalf("unexpected recent project ordering: %+v", bootstrap.RecentProjects)
	}
}

func TestUpdateProjectSettingsPersistsCompatibilitySelection(t *testing.T) {
	app := NewAppWithConfigDir(t.TempDir())
	parentDir := t.TempDir()

	state, err := app.CreateProject(CreateProjectInput{
		Name:          "Compat Lab",
		PackageID:     "tester.compatlab",
		Author:        "Tester",
		Location:      parentDir,
		TargetVersion: supportedVersion,
	})
	if err != nil {
		t.Fatalf("CreateProject returned error: %v", err)
	}

	updated, err := app.UpdateProjectSettings(UpdateProjectSettingsInput{
		ProjectPath: state.Summary.Path,
		Settings: ProjectSettings{
			TargetVersion: supportedVersion,
			Compatibility: CompatibilitySettings{
				Mode:           "selected",
				SelectedModIDs: []string{"mod.alpha", "mod.beta", "mod.alpha"},
				PatchEntries: map[string]CompatibilityPatchEntry{
					"mod.alpha": {
						ModID:       "mod.alpha",
						DisplayName: "Alpha",
						UserEdited:  true,
					},
				},
			},
		},
	})
	if err != nil {
		t.Fatalf("UpdateProjectSettings returned error: %v", err)
	}

	if updated.Settings.Compatibility.Mode != "selected" {
		t.Fatalf("expected compatibility mode to persist, got %s", updated.Settings.Compatibility.Mode)
	}
	if len(updated.Settings.Compatibility.SelectedModIDs) != 2 {
		t.Fatalf("expected duplicate selected mods to be removed, got %+v", updated.Settings.Compatibility.SelectedModIDs)
	}

	reloaded, err := app.GetProjectState(state.Summary.Path)
	if err != nil {
		t.Fatalf("GetProjectState returned error: %v", err)
	}
	if _, ok := reloaded.Settings.Compatibility.PatchEntries["mod.alpha"]; !ok {
		t.Fatalf("expected compatibility patch entry to persist")
	}
}

func TestUpdateGlobalSettingsPersistsThemeSettings(t *testing.T) {
	app := NewAppWithConfigDir(t.TempDir())
	gameRoot := t.TempDir()
	if err := os.MkdirAll(filepath.Join(gameRoot, "Data", "Core"), 0o755); err != nil {
		t.Fatalf("mkdir Core: %v", err)
	}

	bootstrap, err := app.UpdateGlobalSettings(UpdateGlobalSettingsInput{
		GamePath:        gameRoot,
		ScanModsEnabled: true,
		ThemeID:         "archive",
		CustomCSSPath:   "/tmp/custom-theme.css",
	})
	if err != nil {
		t.Fatalf("UpdateGlobalSettings returned error: %v", err)
	}

	if bootstrap.Settings.ThemeID != "archive" {
		t.Fatalf("expected theme to persist, got %s", bootstrap.Settings.ThemeID)
	}
	if bootstrap.Settings.CustomCSSPath != "/tmp/custom-theme.css" {
		t.Fatalf("expected custom css path to persist, got %s", bootstrap.Settings.CustomCSSPath)
	}

	reloaded, err := app.GetAppBootstrap()
	if err != nil {
		t.Fatalf("GetAppBootstrap returned error: %v", err)
	}
	if reloaded.Settings.ThemeID != "archive" {
		t.Fatalf("expected reloaded theme to persist, got %s", reloaded.Settings.ThemeID)
	}
}
