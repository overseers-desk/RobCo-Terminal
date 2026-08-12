#ifndef CRASHLOG_H
#define CRASHLOG_H

#include <QString>

// Arms a last-gasp handler for the fatal signals (SEGV, ABRT, BUS, FPE, ILL):
// it writes the raw backtrace to a file under the given directory and to
// stderr, then re-raises the signal so cores and system crash tooling still
// see it. The frames carry module+offset; addr2line against the binary turns
// them into file:line.
void installCrashLog(const QString &directory);

#endif // CRASHLOG_H
