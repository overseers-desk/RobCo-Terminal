/*
 * Custom wish for the self-contained robco-settings image.
 *
 * A stock wish loads Tk as a shared object at runtime, and zipfs cannot serve
 * a .so to load(). So Tk is linked into this interpreter and registered as a
 * static library: Tcl_StaticLibrary records its init function, and package
 * require resolves against the in-binary code instead of a dlopen.
 *
 * robco-settings has no worker threads and no binary extension beyond Tk
 * itself, so unlike questlog's wish this one links no Thread library.
 */
#include <tcl.h>
#include <tk.h>

#ifdef ROBCO_EMBEDDED_SETTINGS
/*
 * The same interpreter, entered from somewhere else: the Windows terminal
 * links this file, carries the payload zip in its own PE image, and calls
 * RobcoSettingsEmbedded_Main below for `robco-term --settings`. Nothing on
 * the standalone path changes - main() and everything it calls are compiled
 * identically whether or not this is defined.
 *
 * Two things the embedded interpreter knows that the standalone one has to
 * work out: which executable is the terminal (it is this one, so there is no
 * sibling to find) and where the terminal keeps its log. Both are handed to
 * the scripts as ::rcsettings::embedded(...) before anything runs.
 */
#include <stdio.h>
#include <stdlib.h>

static const char *RobcoDiagFile = NULL;
#endif

#if defined(_WIN32) && !defined(ROBCO_EMBEDDED_SETTINGS)
#include <windows.h>
#include <stdio.h>

/*
 * robco-settings answers on stdout as well as in a window: --version prints
 * and exits. On Windows those two answers pull in opposite directions. A
 * console-subsystem binary would open a console window behind every GUI
 * launch; a GUI-subsystem one, which is what gets linked, has no console to
 * print into when it is started from one.
 *
 * So borrow the parent's. Where robco-settings was launched from a terminal
 * and inherited no usable stdout, attach to that terminal's console and point
 * the standard streams at it; where the streams are already connected (a
 * redirection, a pipe), leave them, or the reopen would overwrite the
 * redirection the caller asked for. Double-clicked from Explorer there is no
 * parent console, AttachConsole fails, and nothing is printed - which is the
 * behaviour a windowed launch wants.
 */
static void BorrowParentConsole(void) {
    HANDLE out = GetStdHandle(STD_OUTPUT_HANDLE);
    if (out != NULL && out != INVALID_HANDLE_VALUE) {
        return;
    }
    if (!AttachConsole(ATTACH_PARENT_PROCESS)) {
        return;
    }
    freopen("CONOUT$", "w", stdout);
    freopen("CONOUT$", "w", stderr);
    freopen("CONIN$", "r", stdin);
}
#endif

static int RobcoSettings_AppInit(Tcl_Interp *interp) {
    if (Tcl_Init(interp) != TCL_OK) {
        return TCL_ERROR;
    }
#ifdef ROBCO_EMBEDDED_SETTINGS
    /*
     * Before Tk, because the first thing the window does after Tk is up is
     * ask the terminal for its schema, and lib/dump.tcl reads these to know
     * it need not go looking. A qualified variable cannot be set in a
     * namespace that does not exist yet, so the namespace is made first.
     */
    if (Tcl_Eval(interp, "namespace eval ::rcsettings {}") != TCL_OK) {
        return TCL_ERROR;
    }
    Tcl_SetVar2(interp, "::rcsettings::embedded", "terminal",
            Tcl_GetNameOfExecutable(), TCL_GLOBAL_ONLY);
    if (RobcoDiagFile != NULL) {
        Tcl_SetVar2(interp, "::rcsettings::embedded", "diagfile",
                RobcoDiagFile, TCL_GLOBAL_ONLY);
    }
#endif
    if (Tk_Init(interp) != TCL_OK) {
        return TCL_ERROR;
    }
    Tcl_StaticLibrary(interp, "Tk", Tk_Init, Tk_SafeInit);

    return TCL_OK;
}

/*
 * The standalone image's entry, and only its: embedded in the terminal,
 * the Rust binary owns `main` and this one may not exist beside it
 * (LNK2005 if it does), so the embedded compile keeps only the entry
 * below.
 */
#ifndef ROBCO_EMBEDDED_SETTINGS
int main(int argc, char **argv) {
#ifdef _WIN32
    BorrowParentConsole();
#endif
    /*
     * Mount the appended zip and rewrite argv to source its main.tcl. Without
     * this hook the stubbed image finds no startup script and drops into an
     * interactive wish.
     */
    TclZipfs_AppHook(&argc, &argv);
    Tk_Main(argc, argv, RobcoSettings_AppInit);
    return 0;
}
#endif

#ifdef ROBCO_EMBEDDED_SETTINGS
/*
 * The settings window started from inside the terminal's own executable.
 *
 * zip/ziplen are the payload archive as the linker left it in the image;
 * diagfile is where the terminal wants failures recorded, or NULL to let the
 * scripts choose. argv is passed through to the launcher after the script
 * name, so --version and --selftest reach it as they do standalone.
 *
 * Tk_Main does not return: it ends the process. The int return type is for
 * the mount failure below, which happens before any of Tcl is running.
 */
int RobcoSettingsEmbedded_Main(int argc, char **argv, const void *zip,
        size_t ziplen, const char *diagfile) {
    char **argv2;
    int i;

    RobcoDiagFile = diagfile;

    /*
     * Not TclZipfs_AppHook: its static-build arm goes hunting for a script
     * library archive attached to the executable or the library file, and
     * panics by the library's own name ("Cannot find tcl90s.lib") when
     * there is none. This executable carries the archive as a section of
     * its image instead, mounted by hand below, so the hook's one needed
     * job - Tcl_FindExecutable and the encoding initialisation it brings -
     * is done directly.
     */
    Tcl_FindExecutable(argc > 0 ? argv[0] : NULL);

    if (TclZipfs_MountBuffer(NULL, zip, ziplen, "//zipfs:/app", 1) != TCL_OK) {
        /*
         * This failure is before Tcl: no interpreter to raise it in, no
         * bgerror, and on Windows no console to print it to either. So it
         * goes straight into the terminal's log by hand, which is the same
         * file lib/diag.tcl would have used had there been an interpreter to
         * run it.
         */
        if (diagfile != NULL) {
            FILE *log = fopen(diagfile, "a");
            if (log != NULL) {
                fprintf(log, "robco-settings: cannot mount the embedded "
                        "settings archive (%lu bytes)\n",
                        (unsigned long)ziplen);
                fclose(log);
            }
        }
        fprintf(stderr, "robco-settings: cannot mount the embedded settings "
                "archive\n");
        return 1;
    }

    /*
     * A statically linked interpreter has no script library on disk, so it is
     * told where the mounted one is. If Tcl_Init still fails to find init.tcl
     * on some host, the fallback is to set ::tcl_library in AppInit above
     * before calling Tcl_Init, which skips the environment entirely.
     */
    Tcl_PutEnv("TCL_LIBRARY=//zipfs:/app/tcl_library");
    Tcl_PutEnv("TK_LIBRARY=//zipfs:/app/tk_library");

    argv2 = (char **)malloc((size_t)(argc + 2) * sizeof(char *));
    if (argv2 == NULL) {
        fprintf(stderr, "robco-settings: out of memory\n");
        return 1;
    }
    argv2[0] = argv[0];
    argv2[1] = (char *)"//zipfs:/app/main.tcl";
    for (i = 1; i < argc; i++) {
        argv2[i + 1] = argv[i];
    }
    argv2[argc + 1] = NULL;

    Tk_Main(argc + 1, argv2, RobcoSettings_AppInit);
    return 0;
}
#endif
