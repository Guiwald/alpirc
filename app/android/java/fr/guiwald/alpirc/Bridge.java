package fr.guiwald.alpirc;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.os.Handler;
import android.os.Looper;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.os.VibratorManager;

import android.widget.Toast;

import java.io.File;
import java.util.Locale;
import java.util.TimeZone;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

/**
 * Le pont entre le code Rust d'AlpIRC et Android (ADR 0006) : une file d'événements texte que Rust attend
 * ({@link #take}), et des méthodes statiques que Rust appelle par JNI. Aucune logique IRC ici : seulement ce que Rust
 * ne peut pas faire lui-même (presse-papiers, notifications, vibration, liens, champ de saisie natif, service).
 *
 * <p>Événements (champs séparés par des tabulations) : {@code insets}, {@code send\ttexte}, {@code tab\ttexte},
 * {@code back}, {@code url\tirc://…}, {@code fg\t1|0}, {@code quit}, {@code activity\tdestroyed\tconfig|finish},
 * {@code picked\tjeton[\tchemin]}, {@code log\ttexte}.
 */
public final class Bridge {
    static final LinkedBlockingQueue<String> events = new LinkedBlockingQueue<>();
    static final Handler main = new Handler(Looper.getMainLooper());
    static volatile Context app;
    static volatile AlpircActivity activity;
    static volatile ClipboardManager clipboard;
    /** Canal des notifications des mentions et des messages privés. */
    static final String CHANNEL = "AlpIRC";
    static volatile String notifyChannelDescription = "";

    private Bridge() {}

    static void post(String e) {
        events.offer(e);
    }

    /** Événement suivant (attendu au plus {@code timeoutMs} millisecondes), ou {@code null}. */
    public static String take(long timeoutMs) {
        try {
            return events.poll(timeoutMs, TimeUnit.MILLISECONDS);
        } catch (InterruptedException e) {
            return null;
        }
    }

    /** Marges de l'écran (pixels) : gauche, haut, droite, bas (barre de navigation, clavier, champ natif). */
    public static int[] insets() {
        AlpircActivity a = activity;
        return a == null ? new int[4] : a.insetsForRust();
    }

    /** Affiche ou cache le champ de saisie natif, avec son texte d'aide. */
    public static void setInput(boolean visible, String hint) {
        AlpircActivity a = activity;
        if (a != null) a.runOnUiThread(() -> a.setInputVisible(visible, hint));
    }

    /** Remplace le texte du champ natif (complétion par Tab), curseur à la fin. */
    public static void setInputText(String text) {
        AlpircActivity a = activity;
        if (a != null) a.runOnUiThread(() -> a.setInputText(text));
    }

    /** Libellés des boutons du champ natif (traduits par Rust). */
    public static void setInputLabels(String send, String tab) {
        AlpircActivity a = activity;
        if (a != null) a.runOnUiThread(() -> a.setInputLabels(send, tab));
    }

    /** Ouvre ou ferme le clavier pour un champ dessiné par Rust (boîtes de dialogue). */
    public static void showKeyboard(boolean show) {
        AlpircActivity a = activity;
        if (a != null) a.runOnUiThread(() -> a.showNativeKeyboard(show, 0));
    }

    static int smallIcon(Context c) {
        int id = c.getResources().getIdentifier("ic_stat_alpirc", "drawable", c.getPackageName());
        return id != 0 ? id : c.getApplicationInfo().icon;
    }

    static PendingIntent openApp(Context c, int code) {
        Intent open = new Intent(c, AlpircActivity.class).setAction(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER);
        open.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_SINGLE_TOP);
        return PendingIntent.getActivity(c, code, open, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /** Notification d'une mention ou d'un message privé (canal « AlpIRC ») ; la toucher ouvre AlpIRC. */
    public static void notify(String title, String body) {
        Context c = app;
        if (c == null) return;
        try {
            NotificationManager nm = c.getSystemService(NotificationManager.class);
            if (nm.getNotificationChannel(CHANNEL) == null) {
                NotificationChannel ch = new NotificationChannel(CHANNEL, "AlpIRC", NotificationManager.IMPORTANCE_HIGH);
                if (!notifyChannelDescription.isEmpty()) ch.setDescription(notifyChannelDescription);
                ch.enableVibration(true);
                nm.createNotificationChannel(ch);
            }
            Notification n = new Notification.Builder(c, CHANNEL)
                    .setSmallIcon(smallIcon(c))
                    .setContentTitle(title)
                    .setContentText(body)
                    .setStyle(new Notification.BigTextStyle().bigText(body))
                    .setCategory(Notification.CATEGORY_MESSAGE)
                    .setAutoCancel(true)
                    .setContentIntent(openApp(c, 2))
                    .build();
            // Une notification par conversation (même titre) : la dernière remplace la précédente.
            nm.notify(1000 + (title.hashCode() & 0xffff), n);
        } catch (Exception e) {
            post("log\tnotification : " + e);
        }
    }

    /** Courte vibration (bip). */
    public static void vibrate(int ms) {
        Context c = app;
        if (c == null) return;
        try {
            Vibrator v;
            if (Build.VERSION.SDK_INT >= 31) {
                v = c.getSystemService(VibratorManager.class).getDefaultVibrator();
            } else {
                v = (Vibrator) c.getSystemService(Context.VIBRATOR_SERVICE);
            }
            if (v != null && v.hasVibrator()) v.vibrate(VibrationEffect.createOneShot(Math.max(1, ms), VibrationEffect.DEFAULT_AMPLITUDE));
        } catch (Exception e) {
            post("log\tvibration : " + e);
        }
    }

    /** Ouvre une adresse dans l'appli qui convient (navigateur…). */
    public static void openUrl(String url) {
        Context c = activity != null ? activity : app;
        if (c == null) return;
        main.post(() -> {
            try {
                Intent i = new Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                c.startActivity(i);
            } catch (ActivityNotFoundException e) {
                post("log\taucune appli pour " + url);
            } catch (Exception e) {
                post("log\touverture : " + e);
            }
        });
    }

    public static String getClipboard() {
        ClipboardManager cm = clipboard;
        Context c = app;
        if (cm == null || c == null) return null;
        try {
            ClipData clip = cm.getPrimaryClip();
            if (clip == null || clip.getItemCount() == 0) return null;
            CharSequence t = clip.getItemAt(0).coerceToText(c);
            return t == null ? null : t.toString();
        } catch (Exception e) {
            return null;
        }
    }

    public static void setClipboard(String text) {
        ClipboardManager cm = clipboard;
        if (cm == null) return;
        try {
            cm.setPrimaryClip(ClipData.newPlainText("AlpIRC", text));
        } catch (Exception e) {
            post("log\tpresse-papiers : " + e);
        }
    }

    /**
     * Service de connexion (notification permanente) : {@code active} connexions ouvertes, 0 pour l'arrêter.
     * Les textes sont traduits par Rust.
     */
    public static void service(int active, String title, String text, String quit, String channel, String notifyChannel) {
        if (notifyChannel != null) notifyChannelDescription = notifyChannel;
        ConnectionService.update(active, title, text, quit, channel);
    }

    /** L'appli passe derrière (bouton Retour hors des menus) : les connexions restent ouvertes. */
    public static boolean toBack() {
        AlpircActivity a = activity;
        if (a == null) return false;
        a.runOnUiThread(() -> a.moveTaskToBack(true));
        return true;
    }

    /** Quitte pour de bon : service arrêté, activité fermée, puis le processus. */
    public static void exitApp() {
        main.post(() -> {
            ConnectionService.stopNow();
            AlpircActivity a = activity;
            if (a != null) a.finishAndRemoveTask();
            main.postDelayed(() -> System.exit(0), 300);
        });
    }

    /**
     * Dossier des fichiers reçus (DCC) : {@code Android/data/fr.guiwald.alpirc/files/Download/AlpIRC}, dans le stockage
     * de l'appli (aucune permission), visible par USB ; sinon le dossier privé de l'appli.
     */
    public static String downloadsDir() {
        Context c = app;
        if (c == null) return null;
        File d = c.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS);
        if (d == null) d = new File(c.getFilesDir(), "downloads");
        File out = new File(d, "AlpIRC");
        out.mkdirs();
        return out.getAbsolutePath();
    }

    /** Ouvre le sélecteur de documents ; la réponse arrive en {@code picked\tjeton[\tchemin]}. */
    public static boolean pickFile(int token) {
        AlpircActivity a = activity;
        if (a == null) return false;
        a.runOnUiThread(() -> a.pickFile(token));
        return true;
    }

    /** Ouvre un fichier reçu dans l'appli qui convient ; pour un dossier, affiche son emplacement. */
    public static void openPath(String path) {
        Context c = activity != null ? activity : app;
        if (c == null || path == null) return;
        main.post(() -> {
            File f = new File(path);
            if (f.isDirectory()) {
                Toast.makeText(c, f.getAbsolutePath(), Toast.LENGTH_LONG).show();
                return;
            }
            try {
                Intent i = new Intent(Intent.ACTION_VIEW).setDataAndType(FilesProvider.uriFor(f), FilesProvider.mimeOf(f.getName()));
                i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_ACTIVITY_NEW_TASK);
                c.startActivity(i);
            } catch (Exception e) {
                Toast.makeText(c, f.getAbsolutePath(), Toast.LENGTH_LONG).show();
            }
        });
    }

    /** Décalage de l'heure locale sur UTC (secondes) à un instant. */
    public static int utcOffset(long nowMs) {
        return TimeZone.getDefault().getOffset(nowMs) / 1000;
    }

    /** Langue du système ({@code fr-FR}). */
    public static String locale() {
        return Locale.getDefault().toLanguageTag();
    }

    /** Densité de l'écran (pixels par dp). */
    public static float density() {
        Context c = app;
        return c == null ? 0f : c.getResources().getDisplayMetrics().density;
    }

    /** Relance AlpIRC dans un processus neuf (la boucle de Rust ne se recrée pas dans le même processus). */
    public static boolean restart() {
        AlpircActivity a = activity;
        if (a == null) return false;
        a.runOnUiThread(() -> {
            try {
                Intent r = new Intent(a, RestartActivity.class).putExtra("pid", android.os.Process.myPid());
                Intent f = AlpircActivity.forward;
                if (f != null && Intent.ACTION_VIEW.equals(f.getAction()) && f.getData() != null) {
                    r.putExtra("next", new Intent(f).setClass(a, AlpircActivity.class));
                }
                a.startActivity(r);
            } catch (Exception e) {
                post("log\trelance : " + e);
            }
        });
        return true;
    }
}
