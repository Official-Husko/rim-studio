export namespace main {
	
	export class CustomThemeSummary {
	    id: string;
	    name: string;
	    path: string;
	
	    static createFrom(source: any = {}) {
	        return new CustomThemeSummary(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.path = source["path"];
	    }
	}
	export class GameScanStatus {
	    state: string;
	    message: string;
	    lastUpdated?: string;
	    scannedSources: number;
	    availableModCount: number;
	
	    static createFrom(source: any = {}) {
	        return new GameScanStatus(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.state = source["state"];
	        this.message = source["message"];
	        this.lastUpdated = source["lastUpdated"];
	        this.scannedSources = source["scannedSources"];
	        this.availableModCount = source["availableModCount"];
	    }
	}
	export class ProjectSummary {
	    name: string;
	    path: string;
	    packageId: string;
	    author: string;
	    targetVersion: string;
	    hasRimStudioConfig: boolean;
	
	    static createFrom(source: any = {}) {
	        return new ProjectSummary(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.path = source["path"];
	        this.packageId = source["packageId"];
	        this.author = source["author"];
	        this.targetVersion = source["targetVersion"];
	        this.hasRimStudioConfig = source["hasRimStudioConfig"];
	    }
	}
	export class RecentProject {
	    name: string;
	    path: string;
	    packageId: string;
	    lastOpened: string;
	
	    static createFrom(source: any = {}) {
	        return new RecentProject(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.path = source["path"];
	        this.packageId = source["packageId"];
	        this.lastOpened = source["lastOpened"];
	    }
	}
	export class ScannedModSummary {
	    id: string;
	    name: string;
	    packageId: string;
	    path: string;
	    source: string;
	
	    static createFrom(source: any = {}) {
	        return new ScannedModSummary(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.packageId = source["packageId"];
	        this.path = source["path"];
	        this.source = source["source"];
	    }
	}
	export class GlobalSettings {
	    gamePath: string;
	    scanModsEnabled: boolean;
	    themeId: string;
	    customCssPath: string;
	    cachedModIndex: ScannedModSummary[];
	    recentProjects: RecentProject[];
	
	    static createFrom(source: any = {}) {
	        return new GlobalSettings(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.gamePath = source["gamePath"];
	        this.scanModsEnabled = source["scanModsEnabled"];
	        this.themeId = source["themeId"];
	        this.customCssPath = source["customCssPath"];
	        this.cachedModIndex = this.convertValues(source["cachedModIndex"], ScannedModSummary);
	        this.recentProjects = this.convertValues(source["recentProjects"], RecentProject);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class AppBootstrap {
	    settings: GlobalSettings;
	    recentProjects: RecentProject[];
	    currentProject?: ProjectSummary;
	    scanStatus: GameScanStatus;
	    availableMods: ScannedModSummary[];
	    availableCustomThemes: CustomThemeSummary[];
	
	    static createFrom(source: any = {}) {
	        return new AppBootstrap(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.settings = this.convertValues(source["settings"], GlobalSettings);
	        this.recentProjects = this.convertValues(source["recentProjects"], RecentProject);
	        this.currentProject = this.convertValues(source["currentProject"], ProjectSummary);
	        this.scanStatus = this.convertValues(source["scanStatus"], GameScanStatus);
	        this.availableMods = this.convertValues(source["availableMods"], ScannedModSummary);
	        this.availableCustomThemes = this.convertValues(source["availableCustomThemes"], CustomThemeSummary);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class CompatibilityPatchEntry {
	    modId: string;
	    displayName: string;
	    notes: string;
	    generated: boolean;
	    userEdited: boolean;
	    lastModified?: string;
	
	    static createFrom(source: any = {}) {
	        return new CompatibilityPatchEntry(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.modId = source["modId"];
	        this.displayName = source["displayName"];
	        this.notes = source["notes"];
	        this.generated = source["generated"];
	        this.userEdited = source["userEdited"];
	        this.lastModified = source["lastModified"];
	    }
	}
	export class CompatibilitySettings {
	    mode: string;
	    selectedModIds: string[];
	    patchEntries: Record<string, CompatibilityPatchEntry>;
	    lastGeneratedAt?: string;
	
	    static createFrom(source: any = {}) {
	        return new CompatibilitySettings(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.mode = source["mode"];
	        this.selectedModIds = source["selectedModIds"];
	        this.patchEntries = this.convertValues(source["patchEntries"], CompatibilityPatchEntry, true);
	        this.lastGeneratedAt = source["lastGeneratedAt"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class CreateProjectInput {
	    name: string;
	    packageId: string;
	    author: string;
	    location: string;
	    targetVersion: string;
	
	    static createFrom(source: any = {}) {
	        return new CreateProjectInput(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.packageId = source["packageId"];
	        this.author = source["author"];
	        this.location = source["location"];
	        this.targetVersion = source["targetVersion"];
	    }
	}
	
	export class GameScanSnapshot {
	    scanStatus: GameScanStatus;
	    availableMods: ScannedModSummary[];
	
	    static createFrom(source: any = {}) {
	        return new GameScanSnapshot(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.scanStatus = this.convertValues(source["scanStatus"], GameScanStatus);
	        this.availableMods = this.convertValues(source["availableMods"], ScannedModSummary);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	
	
	export class ProjectSettings {
	    targetVersion: string;
	    notes: string;
	    compatibility: CompatibilitySettings;
	
	    static createFrom(source: any = {}) {
	        return new ProjectSettings(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.targetVersion = source["targetVersion"];
	        this.notes = source["notes"];
	        this.compatibility = this.convertValues(source["compatibility"], CompatibilitySettings);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class ProjectState {
	    summary: ProjectSummary;
	    settings: ProjectSettings;
	
	    static createFrom(source: any = {}) {
	        return new ProjectState(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.summary = this.convertValues(source["summary"], ProjectSummary);
	        this.settings = this.convertValues(source["settings"], ProjectSettings);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	
	
	
	export class UpdateGlobalSettingsInput {
	    gamePath: string;
	    scanModsEnabled: boolean;
	    themeId: string;
	    customCssPath: string;
	
	    static createFrom(source: any = {}) {
	        return new UpdateGlobalSettingsInput(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.gamePath = source["gamePath"];
	        this.scanModsEnabled = source["scanModsEnabled"];
	        this.themeId = source["themeId"];
	        this.customCssPath = source["customCssPath"];
	    }
	}
	export class UpdateProjectSettingsInput {
	    projectPath: string;
	    settings: ProjectSettings;
	
	    static createFrom(source: any = {}) {
	        return new UpdateProjectSettingsInput(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.projectPath = source["projectPath"];
	        this.settings = this.convertValues(source["settings"], ProjectSettings);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

}

