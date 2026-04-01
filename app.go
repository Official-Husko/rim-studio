package main

import (
	"context"
	"encoding/json"
	"encoding/xml"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	wruntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

const (
	appConfigFolderName  = "RimStudio"
	projectConfigRelPath = "Config/rimstudio.project.json"
	supportedVersion     = "1.6"
	eventScanStatus      = "rimstudio:scan-status"
	defaultThemeID       = "rim-neutral"
)

type App struct {
	ctx context.Context

	mu             sync.RWMutex
	configDir      string
	settings       GlobalSettings
	currentProject *ProjectSummary
	scanStatus     GameScanStatus
	availableMods  []ScannedModSummary
	settingsLoaded bool
	scanInFlight   bool
}

type AppBootstrap struct {
	Settings              GlobalSettings       `json:"settings"`
	RecentProjects        []RecentProject      `json:"recentProjects"`
	CurrentProject        *ProjectSummary      `json:"currentProject,omitempty"`
	ScanStatus            GameScanStatus       `json:"scanStatus"`
	AvailableMods         []ScannedModSummary  `json:"availableMods"`
	AvailableCustomThemes []CustomThemeSummary `json:"availableCustomThemes"`
}

type GameScanSnapshot struct {
	ScanStatus    GameScanStatus      `json:"scanStatus"`
	AvailableMods []ScannedModSummary `json:"availableMods"`
}

type GlobalSettings struct {
	GamePath        string              `json:"gamePath"`
	ScanModsEnabled bool                `json:"scanModsEnabled"`
	ThemeID         string              `json:"themeId"`
	CustomCSSPath   string              `json:"customCssPath"`
	CachedModIndex  []ScannedModSummary `json:"cachedModIndex"`
	RecentProjects  []RecentProject     `json:"recentProjects"`
}

type RecentProject struct {
	Name       string `json:"name"`
	Path       string `json:"path"`
	PackageID  string `json:"packageId"`
	LastOpened string `json:"lastOpened"`
}

type ProjectSummary struct {
	Name               string `json:"name"`
	Path               string `json:"path"`
	PackageID          string `json:"packageId"`
	Author             string `json:"author"`
	TargetVersion      string `json:"targetVersion"`
	HasRimStudioConfig bool   `json:"hasRimStudioConfig"`
}

type ProjectState struct {
	Summary  ProjectSummary  `json:"summary"`
	Settings ProjectSettings `json:"settings"`
}

type ProjectSettings struct {
	TargetVersion string                `json:"targetVersion"`
	Notes         string                `json:"notes"`
	Compatibility CompatibilitySettings `json:"compatibility"`
}

type CompatibilitySettings struct {
	Mode            string                             `json:"mode"`
	SelectedModIDs  []string                           `json:"selectedModIds"`
	PatchEntries    map[string]CompatibilityPatchEntry `json:"patchEntries"`
	LastGeneratedAt string                             `json:"lastGeneratedAt,omitempty"`
}

type CompatibilityPatchEntry struct {
	ModID        string `json:"modId"`
	DisplayName  string `json:"displayName"`
	Notes        string `json:"notes"`
	Generated    bool   `json:"generated"`
	UserEdited   bool   `json:"userEdited"`
	LastModified string `json:"lastModified,omitempty"`
}

type CreateProjectInput struct {
	Name          string `json:"name"`
	PackageID     string `json:"packageId"`
	Author        string `json:"author"`
	Location      string `json:"location"`
	TargetVersion string `json:"targetVersion"`
}

type UpdateProjectSettingsInput struct {
	ProjectPath string          `json:"projectPath"`
	Settings    ProjectSettings `json:"settings"`
}

type UpdateGlobalSettingsInput struct {
	GamePath        string `json:"gamePath"`
	ScanModsEnabled bool   `json:"scanModsEnabled"`
	ThemeID         string `json:"themeId"`
	CustomCSSPath   string `json:"customCssPath"`
}

type GameScanStatus struct {
	State             string `json:"state"`
	Message           string `json:"message"`
	LastUpdated       string `json:"lastUpdated,omitempty"`
	ScannedSources    int    `json:"scannedSources"`
	AvailableModCount int    `json:"availableModCount"`
}

type ScannedModSummary struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	PackageID string `json:"packageId"`
	Path      string `json:"path"`
	Source    string `json:"source"`
}

type CustomThemeSummary struct {
	ID   string `json:"id"`
	Name string `json:"name"`
	Path string `json:"path"`
}

type projectFile struct {
	TargetVersion string                `json:"targetVersion"`
	Notes         string                `json:"notes"`
	Compatibility CompatibilitySettings `json:"compatibility"`
}

type aboutXML struct {
	XMLName           xml.Name `xml:"ModMetaData"`
	Name              string   `xml:"name"`
	PackageID         string   `xml:"packageId"`
	Author            string   `xml:"author"`
	SupportedVersions struct {
		Items []string `xml:"li"`
	} `xml:"supportedVersions"`
}

type scanResult struct {
	status GameScanStatus
	mods   []ScannedModSummary
}

func NewApp() *App {
	return &App{
		settings:      defaultGlobalSettings(),
		scanStatus:    defaultScanStatus(),
		availableMods: []ScannedModSummary{},
	}
}

func NewAppWithConfigDir(configDir string) *App {
	app := NewApp()
	app.configDir = configDir
	return app
}

func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	_ = a.ensureStateLoaded()
	a.startBackgroundScanIfConfigured()
}

func (a *App) GetAppBootstrap() (AppBootstrap, error) {
	if err := a.ensureStateLoaded(); err != nil {
		return AppBootstrap{}, err
	}

	customThemes, err := discoverCustomThemes()
	if err != nil {
		return AppBootstrap{}, err
	}

	a.mu.RLock()
	defer a.mu.RUnlock()

	return AppBootstrap{
		Settings:              cloneGlobalSettings(a.settings),
		RecentProjects:        cloneRecentProjects(a.settings.RecentProjects),
		CurrentProject:        cloneProjectSummaryPtr(a.currentProject),
		ScanStatus:            a.scanStatus,
		AvailableMods:         cloneScannedMods(a.availableMods),
		AvailableCustomThemes: cloneCustomThemes(customThemes),
	}, nil
}

func (a *App) CreateProject(input CreateProjectInput) (ProjectState, error) {
	if err := a.ensureStateLoaded(); err != nil {
		return ProjectState{}, err
	}

	input.Name = strings.TrimSpace(input.Name)
	input.PackageID = strings.TrimSpace(input.PackageID)
	input.Author = strings.TrimSpace(input.Author)
	input.Location = strings.TrimSpace(input.Location)
	input.TargetVersion = normalizeTargetVersion(input.TargetVersion)

	if input.Name == "" {
		return ProjectState{}, errors.New("project name is required")
	}
	if input.PackageID == "" {
		return ProjectState{}, errors.New("package ID is required")
	}
	if input.Location == "" {
		return ProjectState{}, errors.New("project location is required")
	}
	if input.TargetVersion != supportedVersion {
		return ProjectState{}, fmt.Errorf("only RimWorld %s is currently supported", supportedVersion)
	}

	parentDir, err := filepath.Abs(input.Location)
	if err != nil {
		return ProjectState{}, fmt.Errorf("resolve project location: %w", err)
	}

	if err := os.MkdirAll(parentDir, 0o755); err != nil {
		return ProjectState{}, fmt.Errorf("create parent directory: %w", err)
	}

	projectDir := filepath.Join(parentDir, sanitizeFolderName(input.Name))
	if _, err := os.Stat(projectDir); err == nil {
		return ProjectState{}, fmt.Errorf("project folder already exists: %s", projectDir)
	} else if !errors.Is(err, os.ErrNotExist) {
		return ProjectState{}, fmt.Errorf("inspect project folder: %w", err)
	}

	if err := createProjectSkeleton(projectDir, input); err != nil {
		return ProjectState{}, err
	}

	state, err := a.loadProjectState(projectDir)
	if err != nil {
		return ProjectState{}, err
	}

	if err := a.recordRecentProject(state.Summary); err != nil {
		return ProjectState{}, err
	}

	a.startBackgroundScanIfConfigured()
	return state, nil
}

func (a *App) OpenProject(projectPath string) (ProjectState, error) {
	if err := a.ensureStateLoaded(); err != nil {
		return ProjectState{}, err
	}

	state, err := a.loadProjectState(projectPath)
	if err != nil {
		return ProjectState{}, err
	}

	if err := a.recordRecentProject(state.Summary); err != nil {
		return ProjectState{}, err
	}

	a.startBackgroundScanIfConfigured()
	return state, nil
}

func (a *App) GetProjectState(projectPath string) (ProjectState, error) {
	if err := a.ensureStateLoaded(); err != nil {
		return ProjectState{}, err
	}

	return a.loadProjectState(projectPath)
}

func (a *App) UpdateProjectSettings(input UpdateProjectSettingsInput) (ProjectState, error) {
	if err := a.ensureStateLoaded(); err != nil {
		return ProjectState{}, err
	}

	projectPath, err := filepath.Abs(strings.TrimSpace(input.ProjectPath))
	if err != nil {
		return ProjectState{}, fmt.Errorf("resolve project path: %w", err)
	}

	if input.Settings.TargetVersion != "" && normalizeTargetVersion(input.Settings.TargetVersion) != supportedVersion {
		return ProjectState{}, fmt.Errorf("only RimWorld %s is currently supported", supportedVersion)
	}

	settings := normalizeProjectSettings(input.Settings)
	if err := writeProjectSettings(projectPath, settings); err != nil {
		return ProjectState{}, err
	}

	state, err := a.loadProjectState(projectPath)
	if err != nil {
		return ProjectState{}, err
	}

	a.mu.Lock()
	a.currentProject = cloneProjectSummaryPtr(&state.Summary)
	a.mu.Unlock()

	return state, nil
}

func (a *App) UpdateGlobalSettings(input UpdateGlobalSettingsInput) (AppBootstrap, error) {
	if err := a.ensureStateLoaded(); err != nil {
		return AppBootstrap{}, err
	}

	settings := defaultGlobalSettings()
	a.mu.RLock()
	settings = cloneGlobalSettings(a.settings)
	a.mu.RUnlock()

	settings.GamePath = strings.TrimSpace(input.GamePath)
	settings.ScanModsEnabled = input.ScanModsEnabled
	settings.ThemeID = normalizeThemeID(input.ThemeID)
	settings.CustomCSSPath = strings.TrimSpace(input.CustomCSSPath)

	if err := validateGamePathOrEmpty(settings.GamePath); err != nil {
		return AppBootstrap{}, err
	}
	if err := a.saveSettings(settings); err != nil {
		return AppBootstrap{}, err
	}

	a.startBackgroundScanIfConfigured()
	return a.GetAppBootstrap()
}

func (a *App) RescanGameData() (GameScanSnapshot, error) {
	if err := a.ensureStateLoaded(); err != nil {
		return GameScanSnapshot{}, err
	}

	a.startBackgroundScanIfConfigured()
	a.mu.RLock()
	defer a.mu.RUnlock()

	return GameScanSnapshot{
		ScanStatus:    a.scanStatus,
		AvailableMods: cloneScannedMods(a.availableMods),
	}, nil
}

func (a *App) GetAvailableMods() ([]ScannedModSummary, error) {
	if err := a.ensureStateLoaded(); err != nil {
		return nil, err
	}

	a.mu.RLock()
	defer a.mu.RUnlock()

	return cloneScannedMods(a.availableMods), nil
}

func (a *App) CloseProject() (AppBootstrap, error) {
	if err := a.ensureStateLoaded(); err != nil {
		return AppBootstrap{}, err
	}

	a.mu.Lock()
	a.currentProject = nil
	a.mu.Unlock()

	return a.GetAppBootstrap()
}

func (a *App) ChooseDirectory(title string, defaultPath string) (string, error) {
	if a.ctx == nil {
		return "", errors.New("desktop dialog is not available before startup")
	}

	path, err := wruntime.OpenDirectoryDialog(a.ctx, wruntime.OpenDialogOptions{
		Title:            title,
		DefaultDirectory: defaultPath,
	})
	if err != nil {
		return "", err
	}

	return path, nil
}

func (a *App) ChooseCSSFile(title string, defaultPath string) (string, error) {
	if a.ctx == nil {
		return "", errors.New("desktop dialog is not available before startup")
	}

	path, err := wruntime.OpenFileDialog(a.ctx, wruntime.OpenDialogOptions{
		Title: title,
		Filters: []wruntime.FileFilter{
			{
				DisplayName: "CSS Stylesheet",
				Pattern:     "*.css",
			},
		},
		DefaultDirectory: filepath.Dir(strings.TrimSpace(defaultPath)),
		DefaultFilename:  filepath.Base(strings.TrimSpace(defaultPath)),
	})
	if err != nil {
		return "", err
	}

	return path, nil
}

func (a *App) ReadCustomCSSFile(path string) (string, error) {
	cleanPath := strings.TrimSpace(path)
	if cleanPath == "" {
		return "", nil
	}

	if strings.ToLower(filepath.Ext(cleanPath)) != ".css" {
		return "", errors.New("custom theme file must be a .css file")
	}

	absPath, err := filepath.Abs(cleanPath)
	if err != nil {
		return "", fmt.Errorf("resolve custom css path: %w", err)
	}

	data, err := os.ReadFile(absPath)
	if err != nil {
		return "", fmt.Errorf("read custom css file: %w", err)
	}

	return string(data), nil
}

func (a *App) ensureStateLoaded() error {
	a.mu.RLock()
	if a.settingsLoaded {
		a.mu.RUnlock()
		return nil
	}
	a.mu.RUnlock()

	settings, err := a.loadSettings()
	if err != nil {
		return err
	}

	a.mu.Lock()
	defer a.mu.Unlock()

	if a.settingsLoaded {
		return nil
	}

	a.settings = settings
	a.availableMods = cloneScannedMods(settings.CachedModIndex)
	a.scanStatus = defaultScanStatus()
	if settings.GamePath != "" {
		a.scanStatus.Message = "Ready to index RimWorld data."
	}
	a.settingsLoaded = true
	return nil
}

func (a *App) startBackgroundScanIfConfigured() {
	if err := a.ensureStateLoaded(); err != nil {
		return
	}

	a.mu.Lock()
	if a.scanInFlight {
		a.mu.Unlock()
		return
	}

	settings := cloneGlobalSettings(a.settings)
	if settings.GamePath == "" {
		a.scanStatus = defaultScanStatus()
		a.mu.Unlock()
		a.emitScanSnapshot()
		return
	}

	a.scanInFlight = true
	a.scanStatus = GameScanStatus{
		State:             "scanning",
		Message:           "Indexing RimWorld base game and DLC data...",
		LastUpdated:       nowRFC3339(),
		ScannedSources:    0,
		AvailableModCount: len(a.availableMods),
	}
	a.mu.Unlock()

	a.emitScanSnapshot()

	go func(settings GlobalSettings) {
		result := runGameScan(settings)

		a.mu.Lock()
		a.scanInFlight = false
		a.scanStatus = result.status
		if settings.ScanModsEnabled {
			a.availableMods = cloneScannedMods(result.mods)
			a.settings.CachedModIndex = cloneScannedMods(result.mods)
		} else {
			a.availableMods = cloneScannedMods(a.settings.CachedModIndex)
			a.scanStatus.AvailableModCount = len(a.availableMods)
		}
		settingsToSave := cloneGlobalSettings(a.settings)
		a.mu.Unlock()

		_ = a.saveSettings(settingsToSave)
		a.emitScanSnapshot()
	}(settings)
}

func (a *App) emitScanSnapshot() {
	if a.ctx == nil {
		return
	}

	a.mu.RLock()
	payload := GameScanSnapshot{
		ScanStatus:    a.scanStatus,
		AvailableMods: cloneScannedMods(a.availableMods),
	}
	a.mu.RUnlock()

	wruntime.EventsEmit(a.ctx, eventScanStatus, payload)
}

func (a *App) loadProjectState(projectPath string) (ProjectState, error) {
	absPath, err := filepath.Abs(strings.TrimSpace(projectPath))
	if err != nil {
		return ProjectState{}, fmt.Errorf("resolve project path: %w", err)
	}

	if err := validateProjectFolder(absPath); err != nil {
		return ProjectState{}, err
	}

	about, err := readAboutFile(absPath)
	if err != nil {
		return ProjectState{}, err
	}

	projectSettings, hasConfig, err := ensureProjectSettings(absPath)
	if err != nil {
		return ProjectState{}, err
	}

	summary := ProjectSummary{
		Name:               fallbackString(strings.TrimSpace(about.Name), filepath.Base(absPath)),
		Path:               absPath,
		PackageID:          strings.TrimSpace(about.PackageID),
		Author:             strings.TrimSpace(about.Author),
		TargetVersion:      projectSettings.TargetVersion,
		HasRimStudioConfig: hasConfig,
	}

	a.mu.Lock()
	a.currentProject = cloneProjectSummaryPtr(&summary)
	a.mu.Unlock()

	return ProjectState{
		Summary:  summary,
		Settings: projectSettings,
	}, nil
}

func (a *App) recordRecentProject(summary ProjectSummary) error {
	a.mu.Lock()
	defer a.mu.Unlock()

	recent := RecentProject{
		Name:       summary.Name,
		Path:       summary.Path,
		PackageID:  summary.PackageID,
		LastOpened: nowRFC3339(),
	}

	projects := make([]RecentProject, 0, len(a.settings.RecentProjects)+1)
	projects = append(projects, recent)
	for _, existing := range a.settings.RecentProjects {
		if samePath(existing.Path, recent.Path) {
			continue
		}
		projects = append(projects, existing)
	}
	if len(projects) > 3 {
		projects = projects[:3]
	}

	a.settings.RecentProjects = projects
	a.currentProject = cloneProjectSummaryPtr(&summary)

	return a.saveSettingsLocked()
}

func (a *App) saveSettings(settings GlobalSettings) error {
	a.mu.Lock()
	defer a.mu.Unlock()

	a.settings = cloneGlobalSettings(settings)
	return a.saveSettingsLocked()
}

func (a *App) saveSettingsLocked() error {
	settingsPath, err := a.settingsFilePath()
	if err != nil {
		return err
	}

	if err := os.MkdirAll(filepath.Dir(settingsPath), 0o755); err != nil {
		return fmt.Errorf("create settings directory: %w", err)
	}

	data, err := json.MarshalIndent(a.settings, "", "  ")
	if err != nil {
		return fmt.Errorf("encode settings: %w", err)
	}

	if err := os.WriteFile(settingsPath, data, 0o644); err != nil {
		return fmt.Errorf("write settings: %w", err)
	}

	return nil
}

func (a *App) loadSettings() (GlobalSettings, error) {
	settingsPath, err := a.settingsFilePath()
	if err != nil {
		return GlobalSettings{}, err
	}

	data, err := os.ReadFile(settingsPath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return defaultGlobalSettings(), nil
		}
		return GlobalSettings{}, fmt.Errorf("read settings: %w", err)
	}

	settings := defaultGlobalSettings()
	if err := json.Unmarshal(data, &settings); err != nil {
		return GlobalSettings{}, fmt.Errorf("decode settings: %w", err)
	}

	settings = cloneGlobalSettings(settings)
	return settings, nil
}

func (a *App) settingsFilePath() (string, error) {
	baseDir := a.configDir
	if baseDir == "" {
		userDir, err := os.UserConfigDir()
		if err != nil {
			return "", fmt.Errorf("locate config directory: %w", err)
		}
		baseDir = filepath.Join(userDir, appConfigFolderName)
	}

	return filepath.Join(baseDir, "settings.json"), nil
}

func defaultGlobalSettings() GlobalSettings {
	return GlobalSettings{
		GamePath:        "",
		ScanModsEnabled: false,
		ThemeID:         defaultThemeID,
		CustomCSSPath:   "",
		CachedModIndex:  []ScannedModSummary{},
		RecentProjects:  []RecentProject{},
	}
}

func defaultScanStatus() GameScanStatus {
	return GameScanStatus{
		State:             "idle",
		Message:           "Set a RimWorld install path in Settings to index game data.",
		LastUpdated:       nowRFC3339(),
		ScannedSources:    0,
		AvailableModCount: 0,
	}
}

func normalizeProjectSettings(settings ProjectSettings) ProjectSettings {
	settings.TargetVersion = normalizeTargetVersion(settings.TargetVersion)
	settings.Notes = strings.TrimSpace(settings.Notes)
	settings.Compatibility.Mode = normalizeCompatibilityMode(settings.Compatibility.Mode)
	settings.Compatibility.SelectedModIDs = uniqueTrimmed(settings.Compatibility.SelectedModIDs)
	if settings.Compatibility.PatchEntries == nil {
		settings.Compatibility.PatchEntries = map[string]CompatibilityPatchEntry{}
	}
	return settings
}

func normalizeTargetVersion(value string) string {
	if strings.TrimSpace(value) == "" {
		return supportedVersion
	}
	return strings.TrimSpace(value)
}

func normalizeCompatibilityMode(value string) string {
	if strings.TrimSpace(value) == "selected" {
		return "selected"
	}
	return "all"
}

func normalizeThemeID(value string) string {
	switch strings.TrimSpace(value) {
	case "workshop", "blueprint", "foundry", "archive", "relay":
		return strings.TrimSpace(value)
	case "ashfall":
		return "workshop"
	case "scribe":
		return "archive"
	case "embers":
		return "foundry"
	default:
		return defaultThemeID
	}
}

func discoverCustomThemes() ([]CustomThemeSummary, error) {
	roots, err := customThemeDirectories()
	if err != nil {
		return nil, err
	}

	themes := []CustomThemeSummary{}
	seen := map[string]struct{}{}

	for _, root := range roots {
		entries, err := os.ReadDir(root)
		if err != nil {
			if errors.Is(err, os.ErrNotExist) {
				continue
			}
			return nil, fmt.Errorf("read theme directory %s: %w", root, err)
		}

		for _, entry := range entries {
			if entry.IsDir() || strings.ToLower(filepath.Ext(entry.Name())) != ".css" {
				continue
			}

			themePath := filepath.Join(root, entry.Name())
			absPath, err := filepath.Abs(themePath)
			if err != nil {
				return nil, fmt.Errorf("resolve theme path %s: %w", themePath, err)
			}
			if _, exists := seen[absPath]; exists {
				continue
			}

			seen[absPath] = struct{}{}
			baseName := strings.TrimSuffix(entry.Name(), filepath.Ext(entry.Name()))
			themes = append(themes, CustomThemeSummary{
				ID:   baseName,
				Name: displayThemeName(baseName),
				Path: absPath,
			})
		}
	}

	sort.Slice(themes, func(i, j int) bool {
		return strings.ToLower(themes[i].Name) < strings.ToLower(themes[j].Name)
	})

	return themes, nil
}

func customThemeDirectories() ([]string, error) {
	exePath, err := os.Executable()
	if err != nil {
		return nil, fmt.Errorf("locate executable path: %w", err)
	}

	paths := []string{
		filepath.Join(filepath.Dir(exePath), "data", "themes"),
	}

	workingDir, err := os.Getwd()
	if err == nil {
		devPath := filepath.Join(workingDir, "data", "themes")
		if devPath != paths[0] {
			paths = append(paths, devPath)
		}
	}

	return paths, nil
}

func displayThemeName(value string) string {
	replacer := strings.NewReplacer("-", " ", "_", " ")
	parts := strings.Fields(replacer.Replace(strings.TrimSpace(value)))
	for index, part := range parts {
		if part == "" {
			continue
		}
		parts[index] = strings.ToUpper(part[:1]) + strings.ToLower(part[1:])
	}
	return strings.Join(parts, " ")
}

func writeProjectSettings(projectPath string, settings ProjectSettings) error {
	projectFilePath := filepath.Join(projectPath, filepath.FromSlash(projectConfigRelPath))
	if err := os.MkdirAll(filepath.Dir(projectFilePath), 0o755); err != nil {
		return fmt.Errorf("create project config directory: %w", err)
	}

	data, err := json.MarshalIndent(projectFile{
		TargetVersion: settings.TargetVersion,
		Notes:         settings.Notes,
		Compatibility: settings.Compatibility,
	}, "", "  ")
	if err != nil {
		return fmt.Errorf("encode project settings: %w", err)
	}

	if err := os.WriteFile(projectFilePath, data, 0o644); err != nil {
		return fmt.Errorf("write project settings: %w", err)
	}

	return nil
}

func ensureProjectSettings(projectPath string) (ProjectSettings, bool, error) {
	projectFilePath := filepath.Join(projectPath, filepath.FromSlash(projectConfigRelPath))
	data, err := os.ReadFile(projectFilePath)
	if err != nil {
		if !errors.Is(err, os.ErrNotExist) {
			return ProjectSettings{}, false, fmt.Errorf("read project settings: %w", err)
		}

		settings := normalizeProjectSettings(ProjectSettings{
			TargetVersion: supportedVersion,
			Compatibility: CompatibilitySettings{
				Mode:           "all",
				SelectedModIDs: []string{},
				PatchEntries:   map[string]CompatibilityPatchEntry{},
			},
		})
		if err := writeProjectSettings(projectPath, settings); err != nil {
			return ProjectSettings{}, false, err
		}
		return settings, true, nil
	}

	stored := projectFile{}
	if err := json.Unmarshal(data, &stored); err != nil {
		return ProjectSettings{}, true, fmt.Errorf("decode project settings: %w", err)
	}

	return normalizeProjectSettings(ProjectSettings{
		TargetVersion: stored.TargetVersion,
		Notes:         stored.Notes,
		Compatibility: stored.Compatibility,
	}), true, nil
}

func createProjectSkeleton(projectDir string, input CreateProjectInput) error {
	directories := []string{
		projectDir,
		filepath.Join(projectDir, "About"),
		filepath.Join(projectDir, "Defs"),
		filepath.Join(projectDir, "Textures"),
		filepath.Join(projectDir, "Config"),
	}
	for _, dir := range directories {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return fmt.Errorf("create directory %s: %w", dir, err)
		}
	}

	aboutPath := filepath.Join(projectDir, "About", "About.xml")
	if err := os.WriteFile(aboutPath, []byte(buildAboutXML(input)), 0o644); err != nil {
		return fmt.Errorf("write About.xml: %w", err)
	}

	return writeProjectSettings(projectDir, normalizeProjectSettings(ProjectSettings{
		TargetVersion: input.TargetVersion,
		Compatibility: CompatibilitySettings{
			Mode:           "all",
			SelectedModIDs: []string{},
			PatchEntries:   map[string]CompatibilityPatchEntry{},
		},
	}))
}

func buildAboutXML(input CreateProjectInput) string {
	return fmt.Sprintf(`<?xml version="1.0" encoding="utf-8"?>
<ModMetaData>
  <name>%s</name>
  <author>%s</author>
  <packageId>%s</packageId>
  <supportedVersions>
    <li>%s</li>
  </supportedVersions>
</ModMetaData>
`, xmlEscape(input.Name), xmlEscape(input.Author), xmlEscape(input.PackageID), xmlEscape(input.TargetVersion))
}

func readAboutFile(projectPath string) (aboutXML, error) {
	aboutPath := filepath.Join(projectPath, "About", "About.xml")
	data, err := os.ReadFile(aboutPath)
	if err != nil {
		return aboutXML{}, fmt.Errorf("read About.xml: %w", err)
	}

	metadata := aboutXML{}
	if err := xml.Unmarshal(data, &metadata); err != nil {
		return aboutXML{}, fmt.Errorf("decode About.xml: %w", err)
	}

	return metadata, nil
}

func validateProjectFolder(projectPath string) error {
	info, err := os.Stat(projectPath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return fmt.Errorf("project folder does not exist: %s", projectPath)
		}
		return fmt.Errorf("inspect project folder: %w", err)
	}
	if !info.IsDir() {
		return fmt.Errorf("project path is not a folder: %s", projectPath)
	}

	aboutPath := filepath.Join(projectPath, "About", "About.xml")
	if _, err := os.Stat(aboutPath); err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return errors.New("selected folder is not a RimWorld mod: missing About/About.xml")
		}
		return fmt.Errorf("inspect About.xml: %w", err)
	}

	return nil
}

func validateGamePathOrEmpty(gamePath string) error {
	if strings.TrimSpace(gamePath) == "" {
		return nil
	}
	corePath := filepath.Join(gamePath, "Data", "Core")
	info, err := os.Stat(corePath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return errors.New("RimWorld install path must contain Data/Core")
		}
		return fmt.Errorf("inspect game path: %w", err)
	}
	if !info.IsDir() {
		return errors.New("RimWorld install path must contain a Data/Core folder")
	}
	return nil
}

func runGameScan(settings GlobalSettings) scanResult {
	if err := validateGamePathOrEmpty(settings.GamePath); err != nil {
		return scanResult{
			status: GameScanStatus{
				State:             "error",
				Message:           err.Error(),
				LastUpdated:       nowRFC3339(),
				ScannedSources:    0,
				AvailableModCount: len(settings.CachedModIndex),
			},
			mods: cloneScannedMods(settings.CachedModIndex),
		}
	}

	dataEntries, err := os.ReadDir(filepath.Join(settings.GamePath, "Data"))
	if err != nil {
		return scanResult{
			status: GameScanStatus{
				State:             "error",
				Message:           fmt.Sprintf("read RimWorld Data directory: %v", err),
				LastUpdated:       nowRFC3339(),
				ScannedSources:    0,
				AvailableModCount: len(settings.CachedModIndex),
			},
			mods: cloneScannedMods(settings.CachedModIndex),
		}
	}

	scannedSources := 0
	for _, entry := range dataEntries {
		if entry.IsDir() {
			scannedSources++
		}
	}

	if !settings.ScanModsEnabled {
		return scanResult{
			status: GameScanStatus{
				State:             "ready",
				Message:           "Indexed base game and DLC data. External mod scanning is disabled.",
				LastUpdated:       nowRFC3339(),
				ScannedSources:    scannedSources,
				AvailableModCount: len(settings.CachedModIndex),
			},
			mods: cloneScannedMods(settings.CachedModIndex),
		}
	}

	mods, err := scanInstalledMods(filepath.Join(settings.GamePath, "Mods"))
	if err != nil {
		return scanResult{
			status: GameScanStatus{
				State:             "error",
				Message:           fmt.Sprintf("scan installed mods: %v", err),
				LastUpdated:       nowRFC3339(),
				ScannedSources:    scannedSources,
				AvailableModCount: len(settings.CachedModIndex),
			},
			mods: cloneScannedMods(settings.CachedModIndex),
		}
	}

	return scanResult{
		status: GameScanStatus{
			State:             "ready",
			Message:           "Indexed base game, DLC data, and installed mod metadata.",
			LastUpdated:       nowRFC3339(),
			ScannedSources:    scannedSources,
			AvailableModCount: len(mods),
		},
		mods: mods,
	}
}

func scanInstalledMods(modsPath string) ([]ScannedModSummary, error) {
	entries, err := os.ReadDir(modsPath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return []ScannedModSummary{}, nil
		}
		return nil, err
	}

	mods := make([]ScannedModSummary, 0, len(entries))
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		projectPath := filepath.Join(modsPath, entry.Name())
		about, err := readAboutFile(projectPath)
		if err != nil {
			continue
		}

		name := fallbackString(strings.TrimSpace(about.Name), entry.Name())
		packageID := strings.TrimSpace(about.PackageID)
		id := fallbackString(packageID, strings.ToLower(strings.ReplaceAll(entry.Name(), " ", ".")))

		mods = append(mods, ScannedModSummary{
			ID:        id,
			Name:      name,
			PackageID: packageID,
			Path:      projectPath,
			Source:    "mod",
		})
	}

	sort.Slice(mods, func(i, j int) bool {
		return strings.ToLower(mods[i].Name) < strings.ToLower(mods[j].Name)
	})

	return mods, nil
}

func cloneGlobalSettings(settings GlobalSettings) GlobalSettings {
	return GlobalSettings{
		GamePath:        settings.GamePath,
		ScanModsEnabled: settings.ScanModsEnabled,
		ThemeID:         normalizeThemeID(settings.ThemeID),
		CustomCSSPath:   settings.CustomCSSPath,
		CachedModIndex:  cloneScannedMods(settings.CachedModIndex),
		RecentProjects:  cloneRecentProjects(settings.RecentProjects),
	}
}

func cloneRecentProjects(items []RecentProject) []RecentProject {
	return append([]RecentProject(nil), items...)
}

func cloneScannedMods(items []ScannedModSummary) []ScannedModSummary {
	return append([]ScannedModSummary(nil), items...)
}

func cloneCustomThemes(items []CustomThemeSummary) []CustomThemeSummary {
	return append([]CustomThemeSummary(nil), items...)
}

func cloneProjectSummaryPtr(summary *ProjectSummary) *ProjectSummary {
	if summary == nil {
		return nil
	}
	copyValue := *summary
	return &copyValue
}

func sanitizeFolderName(name string) string {
	replacer := strings.NewReplacer(
		"/", "-",
		"\\", "-",
		":", "-",
		"*", "",
		"?", "",
		"\"", "",
		"<", "",
		">", "",
		"|", "",
	)
	clean := strings.TrimSpace(replacer.Replace(name))
	if clean == "" {
		return "rimworld-mod"
	}
	return clean
}

func uniqueTrimmed(values []string) []string {
	seen := map[string]struct{}{}
	result := make([]string, 0, len(values))
	for _, value := range values {
		value = strings.TrimSpace(value)
		if value == "" {
			continue
		}
		if _, ok := seen[value]; ok {
			continue
		}
		seen[value] = struct{}{}
		result = append(result, value)
	}
	return result
}

func samePath(a string, b string) bool {
	return filepath.Clean(a) == filepath.Clean(b)
}

func fallbackString(value string, fallback string) string {
	if value != "" {
		return value
	}
	return fallback
}

func xmlEscape(value string) string {
	replacer := strings.NewReplacer(
		"&", "&amp;",
		"<", "&lt;",
		">", "&gt;",
		"\"", "&quot;",
		"'", "&apos;",
	)
	return replacer.Replace(value)
}

func nowRFC3339() string {
	return time.Now().UTC().Format(time.RFC3339)
}
