package fr.guiwald.alpirc;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.graphics.drawable.Icon;
import android.os.Build;
import android.os.IBinder;

/**
 * Service de connexion d'AlpIRC (ADR 0006) : service au premier plan de type « specialUse » qui garde les connexions
 * IRC ouvertes appli en arrière-plan, avec sa notification permanente (« Connecté à 2 réseaux », bouton Quitter).
 * Rust le démarre à la première connexion et le met à jour ({@link Bridge#service}) ; il ne contient aucune logique
 * IRC. Balayer AlpIRC des applis récentes déconnecte et quitte.
 *
 * <p>À faire plus tard : l'option « se déconnecter après N minutes en arrière-plan ».
 */
public class ConnectionService extends Service {
    static final String CHANNEL = "connection";
    static final int ID = 1;
    static volatile ConnectionService instance;
    static volatile boolean wanted;
    static volatile String title = "AlpIRC", text = "", quit = "Quit", channelName = "Connection";

    /** Appelé par Rust (par {@link Bridge#service}) à chaque changement. */
    static void update(int active, String t, String x, String q, String ch) {
        if (t != null) title = t;
        if (x != null) text = x;
        if (q != null) quit = q;
        if (ch != null) channelName = ch;
        wanted = active > 0;
        Bridge.main.post(() -> {
            Context c = Bridge.app;
            if (c == null) return;
            ConnectionService s = instance;
            if (wanted) {
                if (s != null) {
                    s.refresh();
                } else {
                    try {
                        c.startForegroundService(new Intent(c, ConnectionService.class));
                    } catch (Exception e) {
                        Bridge.post("log\tservice de connexion : " + e);
                    }
                }
            } else if (s != null) {
                s.stopForeground(STOP_FOREGROUND_REMOVE);
                s.stopSelf();
            }
        });
    }

    /** Arrêt immédiat (« Quitter »), sur le fil principal. */
    static void stopNow() {
        wanted = false;
        ConnectionService s = instance;
        if (s != null) {
            s.stopForeground(STOP_FOREGROUND_REMOVE);
            s.stopSelf();
        }
    }

    @Override
    public IBinder onBind(Intent i) {
        return null;
    }

    @Override
    public void onCreate() {
        super.onCreate();
        instance = this;
        if (Bridge.app == null) Bridge.app = getApplicationContext();
    }

    @Override
    public int onStartCommand(Intent i, int flags, int id) {
        if (i != null && "quit".equals(i.getAction())) {
            Bridge.post("quit");
            return START_NOT_STICKY;
        }
        // Démarré par startForegroundService : passer au premier plan tout de suite, même pour s'arrêter.
        refresh();
        if (!wanted) {
            stopForeground(STOP_FOREGROUND_REMOVE);
            stopSelf();
        }
        return START_NOT_STICKY;
    }

    /** AlpIRC balayé des applis récentes : il se déconnecte et quitte (ADR 0006). */
    @Override
    public void onTaskRemoved(Intent i) {
        Bridge.post("quit");
        super.onTaskRemoved(i);
    }

    @Override
    public void onDestroy() {
        if (instance == this) instance = null;
        super.onDestroy();
    }

    void refresh() {
        NotificationManager nm = getSystemService(NotificationManager.class);
        NotificationChannel ch = nm.getNotificationChannel(CHANNEL);
        if (ch == null || !channelName.contentEquals(ch.getName())) {
            nm.createNotificationChannel(new NotificationChannel(CHANNEL, channelName, NotificationManager.IMPORTANCE_LOW));
        }
        Intent q = new Intent(this, ConnectionService.class).setAction("quit");
        PendingIntent quitIntent = PendingIntent.getService(this, 1, q, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification n = new Notification.Builder(this, CHANNEL)
                .setSmallIcon(Bridge.smallIcon(this))
                .setContentTitle(title)
                .setContentText(text)
                .setOngoing(true)
                .setShowWhen(false)
                .setCategory(Notification.CATEGORY_SERVICE)
                .setContentIntent(Bridge.openApp(this, 3))
                .addAction(new Notification.Action.Builder(Icon.createWithResource(this, android.R.drawable.ic_menu_close_clear_cancel), quit, quitIntent).build())
                .build();
        try {
            if (Build.VERSION.SDK_INT >= 34) {
                startForeground(ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
            } else {
                startForeground(ID, n);
            }
        } catch (Exception e) {
            Bridge.post("log\tservice de connexion : " + e);
        }
    }
}
