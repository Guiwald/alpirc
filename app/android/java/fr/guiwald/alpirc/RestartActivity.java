package fr.guiwald.alpirc;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.os.Process;

/**
 * Relance d'AlpIRC dans un processus neuf (même méthode qu'Alpagamp). La boucle d'événements de winit ne se crée
 * qu'une fois par processus : si Android recrée l'activité alors que le processus a déjà servi, Rust appelle
 * {@link Bridge#restart}, qui ouvre celle-ci dans un processus à part (« :relance ») ; elle arrête l'ancien processus
 * et rouvre AlpIRC (avec le lien irc:// qui l'avait rouvert, s'il y en a un).
 */
public class RestartActivity extends Activity {
    @Override
    protected void onCreate(Bundle b) {
        super.onCreate(b);
        int pid = getIntent().getIntExtra("pid", -1);
        if (pid > 0 && pid != Process.myPid()) Process.killProcess(pid);
        Intent next = getIntent().getParcelableExtra("next");
        if (next == null) next = new Intent(this, AlpircActivity.class);
        next.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        try {
            startActivity(next);
        } catch (Exception e) {
            startActivity(new Intent(this, AlpircActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        }
        finish();
        Runtime.getRuntime().exit(0);
    }
}
