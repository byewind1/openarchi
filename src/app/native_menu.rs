//! Native macOS menu bar mirroring the in-window egui menu tree.
//!
//! On macOS the menus live in the system menu bar instead of an in-window
//! strip (see `AtelierApp::in_window_menus`). This module builds an `NSMenu`
//! from the same localized keys and command paths, queues clicked actions
//! through a tag registry, and replays them through the normal app methods
//! from `AtelierApp::workspace`.
//!
//! Menu items deliberately carry no key equivalents (except the AppKit-owned
//! Quit/Hide/Minimize): egui must keep receiving ⌘X/⌘C/⌘V/⌘A/⌘Z/⌘S/… as key
//! events for text widgets, and the existing shortcut handlers already
//! implement those commands. Shortcuts are shown as part of the item titles.

use super::*;
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::{NSInteger, NSString};
use std::cell::RefCell;

/// One menu command, replayed into the app on the next frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MenuAction {
    Noop,
    About,
    Quit,
    Language(UiLanguage),
    New,
    Open,
    Save,
    Plan,
    Split,
    Model,
    ShowIn3D,
    ToggleMarquee3D,
    ToggleMarqueeOutside,
    ToggleTrace,
    TraceOptions,
    ToggleGuideLines,
    ToggleConstructionGrid,
    ToggleElementSnap,
    ToggleGridSnap,
    ToggleSnapGuides,
    SnapPointValues,
    GridOptions,
    ToggleAllStories,
    ToggleCutaway,
    ToggleMarqueeFilter,
    ShowSelection3D,
    HideSelection3D,
    ShowAll3D,
    Fit,
    Rebuild,
    Refresh,
    Regenerate,
    Scale,
    PartialStructure,
    Layers,
    PenSets,
    ModelViewDialog,
    GraphicOverrides,
    DesignOptions,
    Renovation,
    Dimensions,
    ToggleOrthogonal,
    AddOnManager,
    ToggleQuickOptionsBar,
    ToggleToolbox,
    ToggleNavigator,
    ToggleInfoBox,
    ToggleQuickOptionsPalette,
    TracePalette,
    ResetWorkspace,
    TeamworkPalette,
    /// Architectural/Documenting tool entries; mirrors the egui tool match.
    Tool(&'static str),
    /// Preview-only command; opens the shared command-preview dialog.
    Command(&'static str),
    /// Palette entry without a dedicated palette implementation.
    Palette(&'static str),
    /// Toolbar entry without a dedicated toolbar implementation.
    Toolbar(&'static str),
}

struct MenuState {
    installed: bool,
    signature: Option<String>,
}

thread_local! {
    /// Tag → action map for the currently installed menu. Cleared on rebuild;
    /// the replaced menu is no longer clickable at that point.
    static REGISTRY: RefCell<Vec<MenuAction>> = const { RefCell::new(Vec::new()) };
    /// Actions clicked since the last frame, drained by `take_pending`.
    static PENDING: RefCell<Vec<MenuAction>> = const { RefCell::new(Vec::new()) };
    /// egui context used to wake the frame loop when a native item is clicked.
    static CTX: RefCell<Option<Context>> = const { RefCell::new(None) };
    static PROXY: RefCell<Option<Retained<MenuProxy>>> = const { RefCell::new(None) };
    static STATE: RefCell<MenuState> = RefCell::new(MenuState {
        installed: false,
        signature: None,
    });
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and the proxy carries
    // no instance variables.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "OpenArchiMenuProxy"]
    struct MenuProxy;

    impl MenuProxy {
        #[unsafe(method(menuAction:))]
        fn menu_action(&self, sender: &NSMenuItem) {
            let tag = sender.tag();
            if tag < 0 {
                return;
            }
            let action = REGISTRY.with_borrow(|registry| registry.get(tag as usize).copied());
            if let Some(action) = action {
                PENDING.with_borrow_mut(|pending| pending.push(action));
                CTX.with_borrow(|ctx| {
                    if let Some(ctx) = ctx {
                        ctx.request_repaint();
                    }
                });
            }
        }
    }
);

fn new_proxy(mtm: MainThreadMarker) -> Retained<MenuProxy> {
    // SAFETY: `init` is valid for an ivars-less NSObject subclass, and we are
    // on the main thread.
    unsafe { msg_send![MenuProxy::alloc(mtm), init] }
}

fn push_action(action: MenuAction) -> NSInteger {
    REGISTRY.with_borrow_mut(|registry| {
        registry.push(action);
        (registry.len() - 1) as NSInteger
    })
}

/// Actions clicked in the native menu since the previous frame.
pub(super) fn take_pending() -> Vec<MenuAction> {
    PENDING.with_borrow_mut(std::mem::take)
}

struct BuiltMenu {
    main: Retained<NSMenu>,
    services: Option<Retained<NSMenu>>,
    windows: Option<Retained<NSMenu>>,
    help: Option<Retained<NSMenu>>,
}

/// Rebuild the native menu when the language, history or any mirrored toggle
/// changed; otherwise keep the installed menu.
pub(super) fn sync(app: &AtelierApp, ctx: &Context) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let signature = app.menu_signature();
    if STATE.with_borrow(|state| {
        state.installed && state.signature.as_deref() == Some(signature.as_str())
    }) {
        return;
    }
    CTX.with_borrow_mut(|slot| *slot = Some(ctx.clone()));
    let built = build_main_menu(app, mtm);
    let nsapp = NSApplication::sharedApplication(mtm);
    nsapp.setMainMenu(Some(&built.main));
    if let Some(services) = &built.services {
        nsapp.setServicesMenu(Some(services));
    }
    if let Some(windows) = &built.windows {
        nsapp.setWindowsMenu(Some(windows));
    }
    if let Some(help) = &built.help {
        nsapp.setHelpMenu(Some(help));
    }
    STATE.with_borrow_mut(|state| {
        state.installed = true;
        state.signature = Some(signature);
    });
}

struct MenuBuilder<'a> {
    mtm: MainThreadMarker,
    app: &'a AtelierApp,
}

impl MenuBuilder<'_> {
    fn tr(&self, key: &str) -> String {
        self.app.language.translate(key).to_owned()
    }

    fn push(&self, action: MenuAction) -> NSInteger {
        push_action(action)
    }

    fn proxy_target(&self) -> Retained<AnyObject> {
        let proxy = PROXY.with_borrow(|slot| slot.clone());
        let proxy = proxy.unwrap_or_else(|| {
            let proxy = new_proxy(self.mtm);
            PROXY.with_borrow_mut(|slot| *slot = Some(proxy.clone()));
            proxy
        });
        // SAFETY: every 'static object can be reinterpreted as AnyObject.
        unsafe { Retained::cast_unchecked::<AnyObject>(proxy) }
    }

    fn app_target(&self) -> Retained<AnyObject> {
        let app = NSApplication::sharedApplication(self.mtm);
        // SAFETY: every 'static object can be reinterpreted as AnyObject.
        unsafe { Retained::cast_unchecked::<AnyObject>(app) }
    }

    /// Item handled by our proxy; replayed as [`MenuAction`] next frame.
    fn item(&self, title: String, action: MenuAction) -> Retained<NSMenuItem> {
        let item = NSMenuItem::new(self.mtm);
        item.setTitle(&NSString::from_str(&title));
        let target = self.proxy_target();
        // SAFETY: the target is our registered menu proxy and the action
        // selector exists on it (see the `MenuProxy` class above).
        unsafe {
            item.setTarget(Some(&target));
            item.setAction(Some(sel!(menuAction:)));
        }
        item.setTag(self.push(action));
        item
    }

    fn labeled(&self, key: &str, action: MenuAction) -> Retained<NSMenuItem> {
        self.item(self.tr(key), action)
    }

    fn checked_text(
        &self,
        title: String,
        action: MenuAction,
        checked: bool,
    ) -> Retained<NSMenuItem> {
        let item = self.item(title, action);
        item.setState(if checked { 1 } else { 0 });
        item
    }

    fn checked(&self, key: &str, action: MenuAction, checked: bool) -> Retained<NSMenuItem> {
        self.checked_text(self.tr(key), action, checked)
    }

    fn disabled_text(&self, title: String) -> Retained<NSMenuItem> {
        let item = self.item(title, MenuAction::Noop);
        item.setEnabled(false);
        item
    }

    fn separator(&self) -> Retained<NSMenuItem> {
        NSMenuItem::separatorItem(self.mtm)
    }

    fn submenu(&self, title: String, items: Vec<Retained<NSMenuItem>>) -> Retained<NSMenuItem> {
        let menu = NSMenu::new(self.mtm);
        menu.setTitle(&NSString::from_str(&title));
        menu.setAutoenablesItems(false);
        for item in items {
            menu.addItem(&item);
        }
        let item = NSMenuItem::new(self.mtm);
        item.setTitle(&NSString::from_str(&title));
        item.setSubmenu(Some(&menu));
        item
    }

    fn submenu_key(&self, key: &str, items: Vec<Retained<NSMenuItem>>) -> Retained<NSMenuItem> {
        self.submenu(self.tr(key), items)
    }

    /// Item dispatched by AppKit itself (hide/show/unhide).
    fn app_item(&self, title: String, selector: Sel) -> Retained<NSMenuItem> {
        let item = NSMenuItem::new(self.mtm);
        item.setTitle(&NSString::from_str(&title));
        let target = self.app_target();
        // SAFETY: the target is the shared NSApplication, which implements
        // the standard hide/unhide action selectors passed in.
        unsafe {
            item.setTarget(Some(&target));
            item.setAction(Some(selector));
        }
        item
    }

    /// Item whose action walks the responder chain (e.g. window minimize).
    fn responder_item(&self, title: String, selector: Sel) -> Retained<NSMenuItem> {
        let item = NSMenuItem::new(self.mtm);
        item.setTitle(&NSString::from_str(&title));
        // SAFETY: `miniaturize:` is dispatched through the responder chain to
        // the key window; no target object is referenced.
        unsafe { item.setAction(Some(selector)) };
        item
    }

    fn key_equivalent(&self, item: &NSMenuItem, key: &str, mask: NSEventModifierFlags) {
        item.setKeyEquivalent(&NSString::from_str(key));
        item.setKeyEquivalentModifierMask(mask);
    }
}

fn preview_items(b: &MenuBuilder, keys: &[&'static str]) -> Vec<Retained<NSMenuItem>> {
    keys.iter()
        .map(|key| b.labeled(key, MenuAction::Command(key)))
        .collect()
}

const PROJECT_PREFERENCES: &[&str] = &[
    "Working Units…",
    "Dimensions…",
    "Calculation Units…",
    "Zones…",
    "Reference Levels…",
    "Location Settings…",
    "Project North…",
    "Survey Point…",
];

const WORK_ENVIRONMENT: &[&str] = &[
    "Appearance…",
    "Keyboard Shortcuts…",
    "Dialog Boxes and Palettes…",
    "Toolbox…",
    "Info Box…",
    "Toolbars…",
    "Menus…",
];

const INTEROPERABILITY: &[&str] = &[
    "Merge…",
    "IFC Project Manager…",
    "IFC Translators…",
    "Import / Export IFC…",
    "SAF Translators…",
    "DXF-DWG Translation Setup…",
    "Classifications & Properties…",
    "Import Point Cloud…",
    "Place Mesh from Survey Data…",
    "Grasshopper Connection…",
    "Bimplus Connection…",
];

const EXTERNAL_CONTENT: &[&str] = &[
    "Hotlink Manager…",
    "Place External Drawing…",
    "Drawing Manager…",
    "Xref Manager…",
];

const LIBRARIES_AND_OBJECTS: &[&str] = &[
    "Library Manager…",
    "BIMcloud Libraries…",
    "New Object…",
    "Open Object…",
    "Save Selection as Object…",
    "Import Blocks…",
    "Import RFA…",
    "Import Rhino / OBJ / FBX…",
];

const PROJECT_INFO: &[&str] = &[
    "Project Info…",
    "Action Center…",
    "Project Notes…",
    "Preview and Credits…",
    "Session Report…",
];

const PLOT_AND_PRINT: &[&str] = &["Plot Setup…", "Plot…", "Page Setup…", "Print…"];

const ARCHITECTURAL_TOOLS: &[&str] = &[
    "Arrow",
    "Marquee",
    "Wall",
    "Wall End",
    "Column",
    "Beam",
    "Slab",
    "Roof",
    "Shell",
    "Stair",
    "Railing",
    "Curtain Wall",
    "Door",
    "Window",
    "Corner-Window",
    "Skylight",
    "Opening",
    "Zone",
    "Mesh",
    "Morph",
    "Object",
    "Lamp",
];

const STRUCTURAL_TOOLS: &[&str] = &["Support", "Link", "Point Load", "Line Load", "Surface Load"];

const MEP_TOOLS: &[&str] = &[
    "Duct Route",
    "Pipe Route",
    "Cable Route",
    "Branch",
    "Fitting",
    "Terminal",
    "Accessory",
    "Equipment",
];

const DOCUMENTING_TOOLS: &[&str] = &[
    "Section",
    "Elevation",
    "Interior Elevation",
    "Detail",
    "Worksheet",
    "Camera",
    "Dimension",
    "Level Dimension",
    "Radial Dimension",
    "Angle Dimension",
    "Text",
    "Label",
    "Grid Element",
    "Fill",
    "Line",
    "Arc / Circle",
    "Polyline",
    "Spline",
    "Hotspot",
    "Figure",
    "Drawing",
];

const PALETTE_NAMES: &[&str] = &[
    "Info Box",
    "Status Bar",
    "Quick Options",
    "Organizer",
    "Navigator Preview",
    "Drawing Manager",
    "Model Check Report",
    "Issue Manager",
    "Change Manager",
    "Design Options",
    "Renovation",
    "Trace & Reference",
    "Favorites",
    "Attributes",
    "Profile Manager",
    "Element Information",
    "Selections",
    "Coordinates",
    "Control Box",
    "Quick Layers",
    "Teamwork Palette",
];

const TOOLBAR_NAMES: &[&str] = &[
    "Standard",
    "Standard for Low-Res Screens",
    "Tools - Architectural",
    "Arrange Elements",
    "Attributes",
    "Drafting Aids",
    "Drag Elements",
    "Edit Elements",
    "3D Visualization",
    "Classic 3D Navigation",
    "MEP Toolbar",
    "Model Check",
    "Structural Engineering",
    "Teamwork",
];

fn build_main_menu(app: &AtelierApp, mtm: MainThreadMarker) -> BuiltMenu {
    // Rebuilding replaces every clickable item, so the tag registry can be
    // rebuilt from scratch. This runs on the main thread, never interleaved
    // with menu clicks.
    REGISTRY.with_borrow_mut(Vec::clear);
    let b = MenuBuilder { mtm, app };
    let project = &app.editor.project;

    // Application menu. AppKit titles it with the process name; items are the
    // About/Services/Hide/Quit template plus the app-level preference menus.
    let mut app_items = vec![
        b.labeled("About OpenArchi", MenuAction::About),
        b.separator(),
        b.submenu_key(
            "Project Preferences",
            preview_items(&b, PROJECT_PREFERENCES),
        ),
        b.submenu_key("Work Environment", preview_items(&b, WORK_ENVIRONMENT)),
        b.separator(),
    ];
    let services_menu = NSMenu::new(mtm);
    services_menu.setTitle(&NSString::from_str(&b.tr("Services")));
    let services_item = NSMenuItem::new(mtm);
    services_item.setTitle(&NSString::from_str(&b.tr("Services")));
    services_item.setSubmenu(Some(&services_menu));
    app_items.push(services_item);
    app_items.push(b.separator());
    let hide = b.app_item(b.tr("Hide OpenArchi"), sel!(hide:));
    b.key_equivalent(&hide, "h", NSEventModifierFlags::Command);
    let hide_others = b.app_item(b.tr("Hide Others"), sel!(hideOtherApplications:));
    b.key_equivalent(
        &hide_others,
        "h",
        NSEventModifierFlags::Command | NSEventModifierFlags::Option,
    );
    let show_all = b.app_item(b.tr("Show All"), sel!(unhideAllApplications:));
    app_items.extend([hide, hide_others, show_all, b.separator()]);
    let quit = b.item(b.tr("Quit OpenArchi"), MenuAction::Quit);
    b.key_equivalent(&quit, "q", NSEventModifierFlags::Command);
    app_items.push(quit);

    // File.
    let file_items = vec![
        b.submenu_key(
            "New",
            vec![
                b.labeled("New Empty Project…", MenuAction::New),
                b.labeled("New & Reset All…", MenuAction::New),
                b.labeled(
                    "Share as Teamwork Project…",
                    MenuAction::Command("Share as Teamwork Project…"),
                ),
            ],
        ),
        b.item(format!("{}     ⌘O", b.tr("Open JSON…")), MenuAction::Open),
        b.item(format!("{}     ⌘S", b.tr("Save JSON…")), MenuAction::Save),
        b.labeled("Save As…", MenuAction::Command("Save As…")),
        b.labeled(
            "Save as Template…",
            MenuAction::Command("Save as Template…"),
        ),
        b.labeled(
            "Save Selection as Module…",
            MenuAction::Command("Save Selection as Module…"),
        ),
        b.labeled(
            "Save to BIMcloud…",
            MenuAction::Command("Save to BIMcloud…"),
        ),
        b.labeled("Publish…", MenuAction::Command("Publish…")),
        b.submenu_key(
            "Recent Projects",
            vec![b.disabled_text(project.name.clone())],
        ),
        b.separator(),
        b.submenu_key("Interoperability", preview_items(&b, INTEROPERABILITY)),
        b.submenu_key("External Content", preview_items(&b, EXTERNAL_CONTENT)),
        b.submenu_key(
            "Libraries and Objects",
            preview_items(&b, LIBRARIES_AND_OBJECTS),
        ),
        b.submenu_key("Info", preview_items(&b, PROJECT_INFO)),
        b.submenu_key("Plot and Print", preview_items(&b, PLOT_AND_PRINT)),
        b.separator(),
        b.labeled("About OpenArchi", MenuAction::About),
    ];

    // Shared Edit catalog; native and in-window paths execute the same backend.
    let mut edit_items = Vec::new();
    for &(group, names) in super::edit_menu::EDIT_GROUPS {
        let items: Vec<_> = names
            .iter()
            .map(|&name| {
                let item = match name {
                    "Suspend Groups" => b.checked(
                        name,
                        MenuAction::Command(name),
                        app.editor.project.edit_state.suspend_groups,
                    ),
                    "Autogroup" => b.checked(
                        name,
                        MenuAction::Command(name),
                        app.editor.project.edit_state.autogroup,
                    ),
                    _ => b.labeled(name, MenuAction::Command(name)),
                };
                item.setTitle(&NSString::from_str(&app.edit_menu_label(name)));
                item.setEnabled(app.edit_command_enabled(name));
                if !super::edit_menu::implemented(name) {
                    item.setToolTip(Some(&NSString::from_str(
                        &b.tr("Unavailable for the current model types"),
                    )));
                }
                item
            })
            .collect();
        if group.is_empty() {
            edit_items.extend(items);
            edit_items.push(b.separator());
        } else {
            edit_items.push(b.submenu_key(group, items));
        }
    }

    // View.
    let mut view_items = vec![
        b.checked_text(
            format!("{}  F2", b.tr("Floor Plan")),
            MenuAction::Plan,
            app.view == View::Plan,
        ),
        b.checked_text(
            b.tr("Split view"),
            MenuAction::Split,
            app.view == View::Split,
        ),
        b.checked_text(
            format!("{}  F3", b.tr("3D Perspective")),
            MenuAction::Model,
            app.view == View::Model,
        ),
        b.labeled("Show selection / Marquee in 3D  F5", MenuAction::ShowIn3D),
        b.separator(),
        b.checked(
            "Show Marquee in 3D",
            MenuAction::ToggleMarquee3D,
            app.show_marquee_3d,
        ),
    ];
    if app.show_marquee_3d {
        view_items.push(b.checked(
            "Show Outside Marquee",
            MenuAction::ToggleMarqueeOutside,
            app.marquee_outside,
        ));
    }
    view_items.extend([
        b.separator(),
        b.checked("Trace", MenuAction::ToggleTrace, app.trace.enabled),
        b.labeled("Trace Options", MenuAction::TraceOptions),
        b.separator(),
        b.submenu_key(
            "Guides",
            vec![
                b.checked(
                    "Snap Guides and Points",
                    MenuAction::ToggleSnapGuides,
                    project.guide_settings.snap_enabled,
                ),
                b.checked(
                    "Guide Lines",
                    MenuAction::ToggleGuideLines,
                    project.guide_settings.guide_lines_enabled,
                ),
                b.separator(),
                b.labeled("Set Snap Point Values…", MenuAction::SnapPointValues),
            ],
        ),
        b.checked(
            "Guide Lines",
            MenuAction::ToggleGuideLines,
            project.guide_settings.guide_lines_enabled,
        ),
        b.checked(
            "Construction Grid Display",
            MenuAction::ToggleConstructionGrid,
            project.grid_settings.show_construction_grid,
        ),
        b.checked(
            "Element Snap",
            MenuAction::ToggleElementSnap,
            app.editor.snap,
        ),
        b.submenu_key(
            "Snap Options",
            vec![
                b.checked(
                    "Element Snap",
                    MenuAction::ToggleElementSnap,
                    app.editor.snap,
                ),
                b.checked(
                    "Grid Snap",
                    MenuAction::ToggleGridSnap,
                    project.grid_settings.grid_snap_enabled,
                ),
                b.checked(
                    "Snap Guides and Points",
                    MenuAction::ToggleSnapGuides,
                    project.guide_settings.snap_enabled,
                ),
            ],
        ),
        b.labeled("Grid & Editing Plane Options", MenuAction::GridOptions),
        b.submenu_key(
            "Elements in 3D View",
            vec![
                b.checked(
                    "Show all stories in 3D",
                    MenuAction::ToggleAllStories,
                    app.all_stories,
                ),
                b.separator(),
                b.checked("3D Cutaway", MenuAction::ToggleCutaway, app.cutaway_enabled),
                b.checked(
                    "Filter and Cut Elements in 3D",
                    MenuAction::ToggleMarqueeFilter,
                    app.show_marquee_3d,
                ),
                b.labeled("Show Selection / Marquee", MenuAction::ShowSelection3D),
                b.labeled("Hide Selection", MenuAction::HideSelection3D),
                b.labeled("Show All", MenuAction::ShowAll3D),
            ],
        ),
        b.submenu_key(
            "3D View Options",
            preview_items(
                &b,
                &[
                    "Projection Settings…",
                    "Perspective",
                    "Axonometry",
                    "Basic",
                    "Vectorial",
                    "Simple Shading",
                    "Technical",
                    "Detailed",
                    "Shadows",
                    "White Model",
                    "Wireframe",
                    "Orbit",
                    "Explore Model",
                ],
            ),
        ),
        b.submenu_key(
            "Zoom and Rebuild",
            vec![
                b.labeled("Fit in Window · F", MenuAction::Fit),
                b.labeled("Rebuild", MenuAction::Rebuild),
                b.labeled("Refresh", MenuAction::Refresh),
                b.labeled("Rebuild & Regenerate", MenuAction::Regenerate),
            ],
        ),
        b.separator(),
        b.submenu_key(
            "Language",
            vec![
                b.checked_text(
                    "English".to_owned(),
                    MenuAction::Language(UiLanguage::English),
                    app.language == UiLanguage::English,
                ),
                b.checked_text(
                    "简体中文".to_owned(),
                    MenuAction::Language(UiLanguage::SimplifiedChinese),
                    app.language == UiLanguage::SimplifiedChinese,
                ),
            ],
        ),
    ]);

    // Design.
    let design_items = vec![
        b.submenu_key(
            "Architectural Tools",
            ARCHITECTURAL_TOOLS
                .iter()
                .map(|name| b.labeled(name, MenuAction::Tool(name)))
                .collect(),
        ),
        b.submenu_key(
            "Structural Engineering Tools",
            preview_items(&b, STRUCTURAL_TOOLS),
        ),
        b.submenu_key("MEP Tools", preview_items(&b, MEP_TOOLS)),
        b.submenu_key(
            "Stories and Connections",
            preview_items(
                &b,
                &[
                    "Story Settings…",
                    "Edit Elements by Stories…",
                    "Trim Elements",
                    "Merge Elements",
                    "Join Wall to Curtain Wall",
                    "Create Opening",
                    "Clear Connections",
                ],
            ),
        ),
        b.submenu_key(
            "Model Operations",
            preview_items(
                &b,
                &[
                    "Solid Element Operations…",
                    "Magic Wand",
                    "Modify Morph",
                    "Modify Railing",
                    "Modify Curtain Wall",
                ],
            ),
        ),
        b.submenu_key(
            "Design Extras",
            preview_items(
                &b,
                &["RoofMaker…", "TrussMaker…", "Grid System…", "Update Zones"],
            ),
        ),
        b.submenu_key(
            "Model Check",
            preview_items(
                &b,
                &[
                    "Collision Detection…",
                    "Physical Quality…",
                    "Model Compare…",
                    "Structural Analytical Model…",
                    "Energy Evaluation…",
                ],
            ),
        ),
    ];

    // Document.
    let document_items = vec![
        b.submenu_key(
            "Documenting Tools",
            DOCUMENTING_TOOLS
                .iter()
                .map(|name| {
                    if *name == "Polyline" {
                        b.labeled(name, MenuAction::Tool(name))
                    } else {
                        b.labeled(name, MenuAction::Command(name))
                    }
                })
                .collect(),
        ),
        b.labeled("Layers…", MenuAction::Layers),
        b.labeled("Floor Plan Scale…", MenuAction::Scale),
        b.labeled("Partial Structure Display…", MenuAction::PartialStructure),
        b.labeled("Pen Sets", MenuAction::PenSets),
        b.labeled("Model View", MenuAction::ModelViewDialog),
        b.labeled("Graphic Overrides", MenuAction::GraphicOverrides),
        b.labeled("Design Options", MenuAction::DesignOptions),
        b.labeled("Renovation", MenuAction::Renovation),
        b.separator(),
        b.submenu_key(
            "Annotation",
            preview_items(
                &b,
                &[
                    "Automatic Dimensioning…",
                    "Label Selected Elements",
                    "Create Patch…",
                    "Spell Checker",
                ],
            ),
        ),
        b.submenu_key(
            "Schedules and Lists",
            preview_items(
                &b,
                &[
                    "Scheme Settings…",
                    "Wall Schedule",
                    "Door Schedule",
                    "Window Schedule",
                    "Object Inventory",
                    "Element Lists",
                    "Component Lists",
                    "Zone Lists",
                    "Project Indexes",
                ],
            ),
        ),
        b.submenu_key(
            "Issues and Changes",
            preview_items(
                &b,
                &[
                    "Issue Manager",
                    "Issue Organizer",
                    "Change Manager",
                    "Keynotes",
                ],
            ),
        ),
        b.submenu_key(
            "Creative Imaging",
            preview_items(
                &b,
                &[
                    "PhotoRender Projection",
                    "Render Marquee Region",
                    "PhotoRendering Settings…",
                    "Surface Painter",
                    "Create Fly-Through…",
                    "Create Sun Study…",
                ],
            ),
        ),
    ];

    // Options.
    let options_items = vec![
        b.checked(
            "Snap to geometry and grid",
            MenuAction::ToggleElementSnap,
            app.editor.snap,
        ),
        b.checked(
            "Orthogonal drawing",
            MenuAction::ToggleOrthogonal,
            app.editor.orthogonal,
        ),
        b.separator(),
        b.submenu_key(
            "Element Attributes",
            vec![
                b.labeled("Layers…", MenuAction::Layers),
                b.labeled("Lines…", MenuAction::Command("Lines…")),
                b.labeled("Pens & Color…", MenuAction::PenSets),
                b.labeled("Fills…", MenuAction::Command("Fills…")),
                b.labeled("Surfaces…", MenuAction::Command("Surfaces…")),
                b.labeled(
                    "Building Materials…",
                    MenuAction::Command("Building Materials…"),
                ),
                b.labeled("Composites…", MenuAction::Command("Composites…")),
                b.labeled("Profile Manager…", MenuAction::Command("Profile Manager…")),
                b.labeled("Zone Categories…", MenuAction::Command("Zone Categories…")),
                b.labeled("Attributes…", MenuAction::Command("Attributes…")),
                b.labeled(
                    "Attribute Manager…",
                    MenuAction::Command("Attribute Manager…"),
                ),
            ],
        ),
        b.submenu_key(
            "Properties and Classification",
            preview_items(
                &b,
                &[
                    "Property Manager…",
                    "Classification Manager…",
                    "Operation Profiles…",
                ],
            ),
        ),
        b.submenu_key(
            "Structural Attributes",
            preview_items(
                &b,
                &[
                    "Load Manager…",
                    "Load Combinations…",
                    "Load Mapping…",
                    "Load Categories…",
                ],
            ),
        ),
        b.submenu_key(
            "Project Preferences",
            vec![
                b.labeled("Working Units…", MenuAction::Command("Working Units…")),
                b.labeled("Dimensions…", MenuAction::Dimensions),
                b.labeled(
                    "Calculation Units…",
                    MenuAction::Command("Calculation Units…"),
                ),
                b.labeled(
                    "Calculation Rules…",
                    MenuAction::Command("Calculation Rules…"),
                ),
                b.labeled("Zones…", MenuAction::Command("Zones…")),
                b.labeled(
                    "Reference Levels…",
                    MenuAction::Command("Reference Levels…"),
                ),
                b.labeled("Legacy…", MenuAction::Command("Legacy…")),
                b.labeled(
                    "Stair Rules and Standards…",
                    MenuAction::Command("Stair Rules and Standards…"),
                ),
                b.labeled(
                    "Structural Analytical Model Units…",
                    MenuAction::Command("Structural Analytical Model Units…"),
                ),
                b.labeled(
                    "Location Settings…",
                    MenuAction::Command("Location Settings…"),
                ),
                b.labeled("Project North…", MenuAction::Command("Project North…")),
                b.labeled("Survey Point…", MenuAction::Command("Survey Point…")),
            ],
        ),
        b.submenu_key(
            "Work Environment",
            preview_items(
                &b,
                &[
                    "Dialog Boxes and Palettes…",
                    "Selection and Element Information…",
                    "Tracker and Coordinate Input…",
                    "Input Constraints and Guides…",
                    "Model Rebuild Options…",
                    "Appearance…",
                    "Data Safety and Integrity…",
                    "Keyboard Shortcuts…",
                    "Toolbox…",
                    "Tool Settings Dialog Boxes…",
                    "Info Box…",
                    "Toolbars…",
                    "Menus…",
                ],
            ),
        ),
        b.separator(),
        b.labeled("Add-On Manager…", MenuAction::AddOnManager),
    ];

    // Teamwork.
    let teamwork_items = vec![
        b.submenu_key(
            "Project",
            preview_items(
                &b,
                &[
                    "Share…",
                    "Migrate Teamwork Project…",
                    "Open / Join Teamwork Project…",
                    "Local Data Manager…",
                ],
            ),
        ),
        b.submenu_key(
            "Traveling",
            preview_items(
                &b,
                &[
                    "Create Travel Pack",
                    "Import Travel Pack",
                    "Leave Teamwork Project",
                ],
            ),
        ),
        b.submenu_key(
            "Send & Receive",
            preview_items(
                &b,
                &[
                    "Send & Receive",
                    "Send Changes",
                    "Receive Changes",
                    "Send & Receive, then Compare",
                    "Reserve Elements…",
                    "Release All",
                ],
            ),
        ),
        b.submenu_key(
            "Messages and Review",
            preview_items(
                &b,
                &[
                    "Assign Selected Elements",
                    "New Message…",
                    "Review Selected Elements…",
                    "Change Tracking",
                ],
            ),
        ),
        b.labeled("Teamwork Palette", MenuAction::TeamworkPalette),
    ];

    // Window.
    let quick_options_label = if app.quick_options_visible {
        b.tr("Hide Quick Options Bar")
    } else {
        b.tr("Show Quick Options Bar")
    };
    let window_items = vec![
        b.item(quick_options_label, MenuAction::ToggleQuickOptionsBar),
        b.responder_item(b.tr("Minimize"), sel!(miniaturize:)),
        b.separator(),
        b.checked("Toolbox", MenuAction::ToggleToolbox, app.show_left),
        b.checked(
            "Navigator & Properties",
            MenuAction::ToggleNavigator,
            app.show_right,
        ),
        b.separator(),
        b.submenu_key(
            "Palettes",
            PALETTE_NAMES
                .iter()
                .map(|name| match *name {
                    "Info Box" => b.checked(name, MenuAction::ToggleInfoBox, app.info_box_visible),
                    "Quick Options" => b.checked(
                        name,
                        MenuAction::ToggleQuickOptionsPalette,
                        app.quick_options_palette_visible,
                    ),
                    "Trace & Reference" => b.labeled(name, MenuAction::TracePalette),
                    "Design Options" => b.labeled(name, MenuAction::DesignOptions),
                    "Renovation" => b.labeled(name, MenuAction::Renovation),
                    _ => b.labeled(name, MenuAction::Palette(name)),
                })
                .collect(),
        ),
        b.separator(),
        b.submenu_key(
            "Toolbars",
            TOOLBAR_NAMES
                .iter()
                .map(|name| b.labeled(name, MenuAction::Toolbar(name)))
                .collect(),
        ),
        b.separator(),
        b.labeled("Reset workspace layout", MenuAction::ResetWorkspace),
    ];

    // Help.
    let help_items = vec![b.labeled("About OpenArchi", MenuAction::About)];

    let (main_menu, services, windows, help) = {
        let main = NSMenu::new(mtm);
        main.setAutoenablesItems(false);
        main.addItem(&b.submenu("OpenArchi".to_owned(), app_items));
        main.addItem(&b.submenu_key("File", file_items));
        main.addItem(&b.submenu_key("Edit", edit_items));
        main.addItem(&b.submenu_key("View", view_items));
        main.addItem(&b.submenu_key("Design", design_items));
        main.addItem(&b.submenu_key("Document", document_items));
        main.addItem(&b.submenu_key("Options", options_items));
        main.addItem(&b.submenu_key("Teamwork", teamwork_items));

        // Extract the Window/Help submenus so AppKit can track them while the
        // tree keeps them retained.
        let window_menu_item = b.submenu_key("Window", window_items);
        let windows_menu = window_menu_item.submenu();
        main.addItem(&window_menu_item);
        let help_menu_item = b.submenu_key("Help", help_items);
        let help_menu = help_menu_item.submenu();
        main.addItem(&help_menu_item);
        (main, Some(services_menu), windows_menu, help_menu)
    };

    BuiltMenu {
        main: main_menu,
        services,
        windows,
        help,
    }
}

impl AtelierApp {
    /// Fingerprint of everything the native menu mirrors; a change triggers a
    /// rebuild on the next frame.
    pub(super) fn menu_signature(&self) -> String {
        format!(
            "{:?}|{:?}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{:?}|{}|{}|{}|{:?}|{}",
            self.language,
            self.view,
            self.editor.undo.is_empty(),
            self.editor.redo.is_empty(),
            self.show_marquee_3d,
            self.marquee_outside,
            self.trace.enabled,
            self.editor.project.guide_settings.guide_lines_enabled,
            self.editor.project.guide_settings.snap_enabled,
            self.editor.project.grid_settings.show_construction_grid,
            self.editor.project.grid_settings.grid_snap_enabled,
            self.editor.snap,
            self.editor.orthogonal,
            self.all_stories,
            self.cutaway_enabled,
            self.show_left,
            self.show_right,
            self.quick_options_visible,
            self.quick_options_palette_visible,
            self.info_box_visible,
            self.editor.selection_ids(),
            self.editor.revision,
            self.editor.clipboard.len(),
            self.edit_dialog.is_some(),
            self.editor.tool,
            self.reshape_session.is_some(),
        )
    }

    /// True while a modal dialog or a special pointer mode blocks menu
    /// commands, mirroring the egui shortcut gating.
    fn menu_gated(&self) -> bool {
        self.edit_dialog.is_some()
            || self.reshape_session.is_some()
            || self.file_action.is_some()
            || self.help
            || self.close_prompt
            || self.recovery_prompt.is_some()
            || self.element_settings.is_some()
            || self.layer_settings.is_some()
            || self.stair_settings_open
            || self.grid_dialog.is_some()
            || self.wall_settings_dialog.is_some()
            || self.command_dialog.is_some()
            || self.plan_option_dialog.is_some()
            || self.transfer_dialog.is_some()
            || self.guide_ui.values.is_some()
            || self.trace.pointer_mode_active()
            || self.orientation_pick_active
            || self.rotated_grid_tool.is_some()
            || self.pick_armed
            || self.inject_armed
            || self.guide_ui.action.is_some()
            || self.guide_ui.drag.is_some()
    }

    /// Replay one clicked native menu item through the same command paths as
    /// the egui menu.
    pub(super) fn perform_menu_action(&mut self, action: MenuAction, ctx: &Context) {
        match action {
            MenuAction::About => {
                self.help = true;
            }
            MenuAction::Quit => {
                if self.editor.dirty() {
                    self.close_prompt = true;
                    self.close_error.clear();
                    self.file_action = None;
                } else {
                    self.allow_close = true;
                    self.discard_recovery_file();
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            MenuAction::Language(language) => {
                if language != self.language {
                    self.language = language;
                    self.language.save_preference();
                    self.editor.status = format!(
                        "{} · {}",
                        self.language.translate("Language"),
                        self.language.label()
                    );
                }
            }
            _ if self.menu_gated() => {}
            MenuAction::Noop => {}
            MenuAction::New => self.file_action = Some(FileAction::New),
            MenuAction::Open => self.file_action = Some(FileAction::Open),
            MenuAction::Save => self.file_action = Some(FileAction::Save),
            MenuAction::Plan => self.view = View::Plan,
            MenuAction::Split => self.view = View::Split,
            MenuAction::Model => self.view = View::Model,
            MenuAction::ShowIn3D => {
                if self.editor.marquee.is_some() {
                    self.show_marquee_3d = true;
                    self.marquee_outside = false;
                    self.fit_marquee();
                } else if !self.editor.selection_ids().is_empty() {
                    self.show_marquee_3d = true;
                    self.fit_selection();
                }
            }
            MenuAction::ToggleMarquee3D => {
                self.show_marquee_3d = !self.show_marquee_3d;
                if self.show_marquee_3d {
                    if self.editor.marquee.is_some() {
                        self.fit_marquee();
                    } else {
                        self.fit_selection();
                    }
                }
            }
            MenuAction::ToggleMarqueeOutside => self.marquee_outside = !self.marquee_outside,
            MenuAction::ToggleTrace => {
                self.trace.enabled = !self.trace.enabled;
                self.trace_toggled();
            }
            MenuAction::TraceOptions | MenuAction::TracePalette => self.open_trace_palette(),
            MenuAction::ToggleGuideLines => {
                let before = self.editor.project.clone();
                self.editor.project.guide_settings.guide_lines_enabled =
                    !self.editor.project.guide_settings.guide_lines_enabled;
                self.editor.commit(before);
            }
            MenuAction::ToggleConstructionGrid => {
                self.editor.project.grid_settings.show_construction_grid =
                    !self.editor.project.grid_settings.show_construction_grid;
            }
            MenuAction::ToggleElementSnap => self.editor.snap = !self.editor.snap,
            MenuAction::ToggleGridSnap => {
                let enabled = !self.editor.project.grid_settings.grid_snap_enabled;
                self.set_grid_snap_enabled(enabled);
            }
            MenuAction::ToggleSnapGuides => {
                let before = self.editor.project.clone();
                self.editor.project.guide_settings.snap_enabled =
                    !self.editor.project.guide_settings.snap_enabled;
                self.editor.commit(before);
            }
            MenuAction::SnapPointValues => {
                let settings = self.editor.project.guide_settings.clone();
                self.guide_ui.values = Some(settings);
                self.guide_ui.error.clear();
            }
            MenuAction::GridOptions => {
                self.grid_dialog = Some(self.editor.project.grid_settings.clone());
                self.grid_dialog_error.clear();
                self.grid_dialog_just_opened = true;
                self.grid_preview_zoom = 25.0;
            }
            MenuAction::ToggleAllStories => self.all_stories = !self.all_stories,
            MenuAction::ToggleCutaway => self.cutaway_enabled = !self.cutaway_enabled,
            MenuAction::ToggleMarqueeFilter => self.show_marquee_3d = !self.show_marquee_3d,
            MenuAction::ShowSelection3D => {
                self.show_marquee_3d = true;
                if self.editor.marquee.is_some() {
                    self.fit_marquee();
                } else {
                    self.fit_selection();
                }
            }
            MenuAction::HideSelection3D => {
                self.editor.status = "3D selection visibility · interface preview".into();
            }
            MenuAction::ShowAll3D => {
                self.editor.status = "All elements shown in 3D".into();
            }
            MenuAction::Fit => self.fit(),
            MenuAction::Rebuild | MenuAction::Refresh => {
                self.editor.status = "View refreshed".into();
            }
            MenuAction::Regenerate => {
                self.editor.status = "Model refresh requested".into();
            }
            MenuAction::Scale => self.open_plan_option_dialog(PlanOptionKind::Scale),
            MenuAction::PartialStructure => {
                self.open_plan_option_dialog(PlanOptionKind::PartialStructure);
            }
            MenuAction::Layers => self.open_layer_settings(),
            MenuAction::PenSets => self.open_plan_option_dialog(PlanOptionKind::PenSets),
            MenuAction::ModelViewDialog => {
                self.open_plan_option_dialog(PlanOptionKind::ModelView);
            }
            MenuAction::GraphicOverrides => {
                self.open_plan_option_dialog(PlanOptionKind::GraphicOverrides);
            }
            MenuAction::DesignOptions => {
                self.open_plan_option_dialog(PlanOptionKind::DesignOptions);
            }
            MenuAction::Renovation => {
                self.open_plan_option_dialog(PlanOptionKind::Renovation);
            }
            MenuAction::Dimensions => {
                self.open_plan_option_dialog(PlanOptionKind::Dimensions);
            }
            MenuAction::ToggleOrthogonal => self.editor.orthogonal = !self.editor.orthogonal,
            MenuAction::AddOnManager => {
                self.editor.status = "Add-On Manager · interface preview".into();
            }
            MenuAction::ToggleQuickOptionsBar => self.toggle_quick_options_bar(),
            MenuAction::ToggleToolbox => self.show_left = !self.show_left,
            MenuAction::ToggleNavigator => self.show_right = !self.show_right,
            MenuAction::ToggleInfoBox => self.toggle_info_box_visibility(),
            MenuAction::ToggleQuickOptionsPalette => self.toggle_quick_options_palette(),
            MenuAction::ResetWorkspace => {
                self.show_left = true;
                self.show_right = true;
                self.quick_options_visible = true;
                self.quick_options_palette_visible = false;
                self.info_box_visible = true;
                self.view = View::Plan;
                self.split = 0.53;
            }
            MenuAction::TeamworkPalette => {
                self.editor.status = "Teamwork Palette · standalone preview".into();
            }
            MenuAction::Tool(name) => self.activate_named_tool(name),
            MenuAction::Command(command) => self.open_command_preview(command.to_owned()),
            MenuAction::Palette(name) => self.editor.status = format!("Palette · {name}"),
            MenuAction::Toolbar(name) => self.editor.status = format!("Toolbar · {name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> AtelierApp {
        AtelierApp::from_renderer(None, "Headless test".into())
    }

    #[test]
    fn menu_actions_roundtrip_through_the_registry() {
        REGISTRY.with_borrow_mut(Vec::clear);
        let actions = [
            MenuAction::About,
            MenuAction::Tool("Wall"),
            MenuAction::Command("Merge…"),
            MenuAction::Language(UiLanguage::SimplifiedChinese),
        ];
        let tags: Vec<NSInteger> = actions.iter().map(|action| push_action(*action)).collect();
        for (tag, expected) in tags.iter().zip(&actions) {
            assert_eq!(
                REGISTRY.with_borrow(|registry| registry.get(*tag as usize).copied()),
                Some(*expected)
            );
        }
        REGISTRY.with_borrow_mut(Vec::clear);
    }

    #[test]
    fn menu_signature_reflects_language_and_mirrored_toggles() {
        let mut app = app();
        let english = app.menu_signature();
        app.language = UiLanguage::SimplifiedChinese;
        assert_ne!(app.menu_signature(), english);
        app.language = UiLanguage::English;
        app.show_marquee_3d = true;
        assert_ne!(app.menu_signature(), english);
        assert_eq!(app.menu_signature(), app.menu_signature());
    }

    #[test]
    fn modal_dialogs_gate_menu_commands_but_not_about() {
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        let mut app = app();
        app.file_action = Some(FileAction::Open);
        let snap = app.editor.snap;
        app.perform_menu_action(MenuAction::ToggleElementSnap, &ctx);
        assert_eq!(app.editor.snap, snap, "modal must gate menu commands");
        app.perform_menu_action(MenuAction::About, &ctx);
        assert!(app.help, "About must stay available while a modal is open");
    }

    #[test]
    fn language_menu_action_updates_the_language_and_preference_flag() {
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        let mut app = app();
        app.perform_menu_action(MenuAction::Language(UiLanguage::SimplifiedChinese), &ctx);
        assert_eq!(app.language, UiLanguage::SimplifiedChinese);
        app.perform_menu_action(MenuAction::Language(UiLanguage::SimplifiedChinese), &ctx);
        assert_eq!(app.language, UiLanguage::SimplifiedChinese);
    }
}
