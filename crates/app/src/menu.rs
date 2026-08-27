//! The application menu, where the platform draws one.
//!
//! macOS puts an application's menu on the screen rather than in its window,
//! and expects Command-comma to open its settings; a Mac reaches for the
//! menu bar for the things that belong to the program rather than to what is
//! on the glass. This is that menu: About, Settings, Services, Hide, Hide
//! Others, Show All and Quit. Every item but Settings is one a Mac
//! application is expected to carry, and all of them but Settings and Quit
//! are answered by AppKit itself.
//!
//! It opens nothing and closes nothing itself. Settings and Quit post a
//! [`ShellEvent`] and stop there, because both are decisions the menu cannot
//! see: which window a settings press means, and whether the loop still has
//! windows to close. The shell answers both, the same way it answers the
//! seam drag that arrives over the same channel.
//!
//! Off macOS both entry points are visible no-ops, so [`crate::shell`] calls
//! them with no `cfg` of its own -- the shape [`crate::settings_embed`] uses
//! for the settings interpreter.

#[cfg(target_os = "macos")]
mod platform {
    use std::sync::OnceLock;

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, Sel};
    use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
    use objc2_app_kit::{
        NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem, NSRunningApplication,
    };
    use objc2_foundation::{ns_string, MainThreadMarker, NSString};
    use winit::event_loop::{EventLoopBuilder, EventLoopProxy};
    use winit::platform::macos::EventLoopBuilderExtMacOS;

    use crate::shell::ShellEvent;

    /// The menu's way back to the loop.
    ///
    /// A static because an Objective-C action is an `extern "C"` function the
    /// runtime calls with a receiver and a sender and nothing else: there is
    /// nowhere to thread a proxy through but a class ivar or a global, and
    /// the object holding an ivar would have to live as long as the process
    /// anyway (see the leak at the end of [`install`]).
    ///
    /// Written once, from `install`, on the main thread, before the menu that
    /// reads it exists.
    static SHELL: OnceLock<EventLoopProxy<ShellEvent>> = OnceLock::new();

    /// Hand one event to the shell, shrugging at a loop that has already
    /// stopped: a menu click during teardown is a click with nothing left to
    /// act on.
    fn post(event: ShellEvent) {
        match SHELL.get() {
            Some(proxy) => {
                let _ = proxy.send_event(event);
            }
            None => log::debug!("the application menu fired before it was wired up"),
        }
    }

    declare_class!(
        /// The object this program's own two items name as their target.
        ///
        /// A class because AppKit dispatches a menu item by selector and
        /// something has to answer ours; About, Hide and Show All need no
        /// such thing, their selectors being answered by `NSApplication` up
        /// the responder chain.
        struct MenuTarget;

        // SAFETY:
        // - `NSObject` imposes no subclassing requirements.
        // - The class carries no Rust state, so interior mutability is the
        //   weakest claim that holds.
        // - `MenuTarget` does not implement `Drop`.
        unsafe impl ClassType for MenuTarget {
            type Super = NSObject;
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "RobcoTermMenuTarget";
        }

        // No ivars: the proxy is the `SHELL` static above.
        impl DeclaredClass for MenuTarget {}

        // SAFETY: both take the (receiver, selector, sender) shape AppKit
        // calls an action with, and neither touches the sender.
        unsafe impl MenuTarget {
            #[method(robcoOpenSettings:)]
            fn open_settings(&self, _sender: Option<&AnyObject>) {
                post(ShellEvent::OpenSettings);
            }

            #[method(robcoQuit:)]
            fn quit(&self, _sender: Option<&AnyObject>) {
                post(ShellEvent::Quit);
            }
        }

        unsafe impl NSObjectProtocol for MenuTarget {}
    );

    /// Keep winit from putting its own menu bar up.
    ///
    /// Called on the builder because winit installs its menu inside
    /// `applicationDidFinishLaunching:`, a few lines before the event that
    /// [`install`] rides in on; a menu of ours put up first would be
    /// replaced. Ours is not an amendment of winit's either: its Quit item
    /// calls AppKit's `terminate:`, and this program's Quit has to reach the
    /// loop instead ([`ShellEvent::Quit`]).
    pub fn suppress_default(builder: &mut EventLoopBuilder<ShellEvent>) {
        builder.with_default_menu(false);
    }

    /// Put the menu up. Once per process, from the shell's `resumed`, where
    /// the application exists and has finished launching.
    pub fn install(identity: &str, proxy: EventLoopProxy<ShellEvent>) {
        let Some(mtm) = MainThreadMarker::new() else {
            // Unreachable through `resumed`: winit refuses to build an event
            // loop off the main thread. Not a reason to end the process.
            log::warn!("the application menu was asked for off the main thread");
            return;
        };
        if SHELL.set(proxy).is_err() {
            log::debug!("the application menu is already up");
            return;
        }

        let app = NSApplication::sharedApplication(mtm);
        // SAFETY: `MenuTarget` overrides no initialiser, so `NSObject`'s own
        // `init` is the whole of it.
        let target: Retained<MenuTarget> = unsafe { msg_send_id![mtm.alloc::<MenuTarget>(), init] };

        // What the Dock and the Finder call this program: the bundle's name
        // where it runs from one, the executable's where it does not. Read
        // rather than written down, `Info.plist` being that name's home.
        let name = unsafe { NSRunningApplication::currentApplication().localizedName() }
            .unwrap_or_else(|| NSString::from_str(identity));

        let menubar = NSMenu::new(mtm);
        let app_item = NSMenuItem::new(mtm);
        menubar.addItem(&app_item);

        // Untitled on purpose: AppKit draws the first submenu's name from the
        // bundle and ignores whatever this carries.
        let app_menu = NSMenu::new(mtm);

        let about = item(
            mtm,
            &ns_string!("About ").stringByAppendingString(&name),
            Some(sel!(orderFrontStandardAboutPanel:)),
            ns_string!(""),
        );
        let settings = item(
            mtm,
            ns_string!("Settings…"),
            Some(sel!(robcoOpenSettings:)),
            ns_string!(","),
        );
        // The system's own menu of what other applications offer to do with a
        // selection. AppKit fills it; this hands it the empty submenu to fill.
        let services_menu = NSMenu::new(mtm);
        let services = item(mtm, ns_string!("Services"), None, ns_string!(""));
        services.setSubmenu(Some(&services_menu));
        let hide = item(
            mtm,
            &ns_string!("Hide ").stringByAppendingString(&name),
            Some(sel!(hide:)),
            ns_string!("h"),
        );
        let hide_others = item(
            mtm,
            ns_string!("Hide Others"),
            Some(sel!(hideOtherApplications:)),
            ns_string!("h"),
        );
        // The one item whose shortcut is not Command alone.
        hide_others.setKeyEquivalentModifierMask(
            NSEventModifierFlags::NSEventModifierFlagOption
                | NSEventModifierFlags::NSEventModifierFlagCommand,
        );
        let show_all = item(
            mtm,
            ns_string!("Show All"),
            Some(sel!(unhideAllApplications:)),
            ns_string!(""),
        );
        let quit = item(
            mtm,
            &ns_string!("Quit ").stringByAppendingString(&name),
            Some(sel!(robcoQuit:)),
            ns_string!("q"),
        );

        // SAFETY: `MenuTarget` implements both selectors, and the object is
        // leaked below, so it outlives the items pointing at it.
        unsafe {
            settings.setTarget(Some(&target));
            quit.setTarget(Some(&target));
        }

        app_menu.addItem(&about);
        app_menu.addItem(&NSMenuItem::separatorItem(mtm));
        app_menu.addItem(&settings);
        app_menu.addItem(&NSMenuItem::separatorItem(mtm));
        app_menu.addItem(&services);
        app_menu.addItem(&NSMenuItem::separatorItem(mtm));
        app_menu.addItem(&hide);
        app_menu.addItem(&hide_others);
        app_menu.addItem(&show_all);
        app_menu.addItem(&NSMenuItem::separatorItem(mtm));
        app_menu.addItem(&quit);
        app_item.setSubmenu(Some(&app_menu));

        // SAFETY: a menu built here and held by the menu bar below, which is
        // what AppKit fills once it is named.
        unsafe { app.setServicesMenu(Some(&services_menu)) };
        app.setMainMenu(Some(&menubar));

        // A menu item holds its target weakly, AppKit declaring the property
        // so, and this one has to answer for as long as the menu bar stands,
        // which is as long as the process. Dropping it here would leave two
        // items aimed at freed memory, which is what `setTarget:` is unsafe
        // about.
        std::mem::forget(target);
    }

    /// One item. Command is the implicit modifier of a key equivalent, so an
    /// item wanting Command alone sets no mask; an empty key is an item with
    /// no shortcut at all.
    fn item(
        mtm: MainThreadMarker,
        title: &NSString,
        action: Option<Sel>,
        key: &NSString,
    ) -> Retained<NSMenuItem> {
        // SAFETY: a fresh allocation, two live strings, and a selector either
        // this crate's class or `NSApplication` answers.
        unsafe { NSMenuItem::initWithTitle_action_keyEquivalent(mtm.alloc(), title, action, key) }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use winit::event_loop::{EventLoopBuilder, EventLoopProxy};

    use crate::shell::ShellEvent;

    /// Nothing to suppress: macOS is the only platform that draws a menu bar
    /// for an application that never asked for one.
    pub fn suppress_default(_builder: &mut EventLoopBuilder<ShellEvent>) {}

    /// No application menu here. The settings window is reached by the
    /// right-click that reaches it everywhere, and the process ends when its
    /// last window closes.
    pub fn install(_identity: &str, _proxy: EventLoopProxy<ShellEvent>) {}
}

pub use platform::{install, suppress_default};
