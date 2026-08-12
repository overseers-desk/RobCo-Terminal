#include "crashlog.h"

#include <QDir>

#include <climits>
#include <csignal>
#include <cstring>
#include <ctime>
#include <execinfo.h>
#include <fcntl.h>
#include <unistd.h>

namespace {

// Everything the handler touches is precomputed: a signal handler on a
// corrupted heap may call nothing that allocates.
char logPath[PATH_MAX];

void writeAll(int fd, const char *text)
{
    size_t remaining = strlen(text);
    while (remaining > 0) {
        const ssize_t put = write(fd, text, remaining);
        if (put <= 0)
            return;
        text += put;
        remaining -= size_t(put);
    }
}

void handleFatalSignal(int signalNumber)
{
    void *frames[64];
    const int depth = backtrace(frames, 64);

    char head[64];
    int at = 0;
    const char *label = "fatal signal ";
    for (const char *c = label; *c; ++c)
        head[at++] = *c;
    if (signalNumber >= 10)
        head[at++] = char('0' + signalNumber / 10);
    head[at++] = char('0' + signalNumber % 10);
    head[at++] = '\n';
    head[at] = '\0';

    const int fd = open(logPath, O_CREAT | O_WRONLY | O_TRUNC, 0644);
    if (fd >= 0) {
        writeAll(fd, head);
        backtrace_symbols_fd(frames, depth, fd);
        close(fd);
    }
    writeAll(2, head);
    backtrace_symbols_fd(frames, depth, 2);

    signal(signalNumber, SIG_DFL);
    raise(signalNumber);
}

} // namespace

void installCrashLog(const QString &directory)
{
    QDir().mkpath(directory);
    const QString path = directory + QStringLiteral("/crash-%1-%2.log")
            .arg(time(nullptr)).arg(getpid());
    if (path.toLocal8Bit().size() >= int(sizeof(logPath)))
        return;
    strcpy(logPath, path.toLocal8Bit().constData());

    // First call loads libgcc's unwinder; in the handler that load, and the
    // allocation it does, must already be behind us.
    void *warm[2];
    backtrace(warm, 2);

    for (const int signalNumber : {SIGSEGV, SIGABRT, SIGBUS, SIGFPE, SIGILL})
        signal(signalNumber, handleFatalSignal);
}
