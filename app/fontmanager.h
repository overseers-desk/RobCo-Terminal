#ifndef FONTMANAGER_H
#define FONTMANAGER_H

#include <QObject>
#include <QStringList>
#include <QHash>
#include <QSet>

#include "fontlistmodel.h"

class FontManager : public QObject
{
    Q_OBJECT
    Q_PROPERTY(FontListModel *fontList READ fontList CONSTANT)
    Q_PROPERTY(FontListModel *filteredFontList READ filteredFontList NOTIFY filteredFontListChanged)
    Q_PROPERTY(FontListModel *lowResolutionFontList READ lowResolutionFontList CONSTANT)
    Q_PROPERTY(int fontSource READ fontSource WRITE setFontSource NOTIFY fontSourceChanged)
    Q_PROPERTY(int rasterization READ rasterization WRITE setRasterization NOTIFY rasterizationChanged)
    Q_PROPERTY(QString fontName READ fontName WRITE setFontName NOTIFY fontNameChanged)
    Q_PROPERTY(qreal fontScaling READ fontScaling WRITE setFontScaling NOTIFY fontScalingChanged)
    Q_PROPERTY(qreal fontWidth READ fontWidth WRITE setFontWidth NOTIFY fontWidthChanged)
    Q_PROPERTY(qreal lineSpacing READ lineSpacing WRITE setLineSpacing NOTIFY lineSpacingChanged)
    Q_PROPERTY(qreal baseFontScaling READ baseFontScaling WRITE setBaseFontScaling NOTIFY baseFontScalingChanged)
    Q_PROPERTY(bool lowResolutionFont READ lowResolutionFont NOTIFY lowResolutionFontChanged)

public:
    explicit FontManager(QObject *parent = nullptr);

    Q_INVOKABLE void refresh();

    FontListModel *fontList();
    FontListModel *filteredFontList();
    FontListModel *lowResolutionFontList();

    Q_INVOKABLE QVariantMap fontByName(const QString &name) const;

    // The LED strip's glyph raster: the text drawn one image pixel per font
    // pixel with antialiasing off, delivered as a data URL. Rendered on the
    // CPU so the lamp grid reads the font's own bitmap whatever the screen's
    // scale factor; a Text item rasterises at device resolution and hands the
    // strip grey-edged glyphs on scaled screens.
    Q_INVOKABLE QString ledTextImage(const QString &family,
                                     int pixelSize,
                                     const QString &text) const;

    int fontSource() const;
    void setFontSource(int fontSource);

    int rasterization() const;
    void setRasterization(int rasterization);

    QString fontName() const;
    void setFontName(const QString &fontName);

    qreal fontScaling() const;
    void setFontScaling(qreal fontScaling);

    qreal fontWidth() const;
    void setFontWidth(qreal fontWidth);

    qreal lineSpacing() const;
    void setLineSpacing(qreal lineSpacing);

    qreal baseFontScaling() const;
    void setBaseFontScaling(qreal baseFontScaling);

    bool lowResolutionFont() const;

signals:
    void fontSourceChanged();
    void rasterizationChanged();
    void fontNameChanged();
    void fontScalingChanged();
    void fontWidthChanged();
    void lineSpacingChanged();
    void baseFontScalingChanged();
    void lowResolutionFontChanged();
    void filteredFontListChanged();

    void terminalFontChanged(QString fontFamily,
                             int pixelSize,
                             int lineSpacing,
                             qreal screenScaling,
                             qreal fontWidth,
                             QString fallbackFontFamily,
                             bool lowResolutionFont);

private:
    QStringList retrieveMonospaceFonts();
    void populateBundledFonts();
    void populateSystemFonts();
    void addBundledFont(const QString &name,
                        const QString &text,
                        const QString &source,
                        qreal baseWidth,
                        int pixelSize,
                        bool lowResolutionFont,
                        const QString &fallbackName = QString());
    void setFontSubstitutions(const QString &family, const QStringList &substitutes);
    void removeFontSubstitution(const QString &family);
    void updateFilteredFonts();
    void updateComputedFont();
    const FontEntry *findFontByName(const QString &name) const;
    QString resolveFontFamily(const QString &sourcePath);
    qreal computeBaseWidth(const QString &family, int pixelSize, qreal fallbackWidth) const;

    FontListModel m_fontListModel;
    FontListModel m_filteredFontListModel;
    FontListModel m_lowResolutionFontListModel;
    QVector<FontEntry> m_allFonts;

    int m_fontSource = 0;
    int m_rasterization = 0;
    QString m_fontName = QStringLiteral("TERMINESS_SCALED");
    qreal m_fontScaling = 1.0;
    qreal m_fontWidth = 1.0;
    qreal m_lineSpacing = 0.1;
    qreal m_baseFontScaling = 0.75;
    bool m_lowResolutionFont = false;

    QHash<QString, QString> m_loadedFamilies;
    QSet<QString> m_bundledFamilies;
};

#endif // FONTMANAGER_H
