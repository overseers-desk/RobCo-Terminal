#ifndef MODIFIERWATCHER_H
#define MODIFIERWATCHER_H

#include <QObject>

class ModifierWatcher : public QObject
{
    Q_OBJECT

public:
    explicit ModifierWatcher(QObject *parent = nullptr);

signals:
    void chordModifierReleased();

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;
};

#endif // MODIFIERWATCHER_H
