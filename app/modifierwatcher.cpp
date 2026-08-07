#include "modifierwatcher.h"

#include <QEvent>
#include <QKeyEvent>

ModifierWatcher::ModifierWatcher(QObject *parent)
    : QObject(parent)
{
}

bool ModifierWatcher::eventFilter(QObject *watched, QEvent *event)
{
    Q_UNUSED(watched);

    if (event->type() == QEvent::KeyRelease) {
        const QKeyEvent *keyEvent = static_cast<QKeyEvent *>(event);
#if defined(Q_OS_MAC)
        const int chordModifier = Qt::Key_Meta;
#else
        const int chordModifier = Qt::Key_Alt;
#endif
        if (keyEvent->key() == chordModifier && !keyEvent->isAutoRepeat())
            emit chordModifierReleased();
    }

    // Observe only: consuming the event would bypass QShortcutMap and
    // swallow keys bound elsewhere.
    return false;
}
