package fr.guiwald.alpirc;

import android.Manifest;
import android.app.NativeActivity;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.graphics.Color;
import android.graphics.PixelFormat;
import android.graphics.Typeface;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.LayerDrawable;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.text.InputType;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowManager;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;

import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.HashMap;

/**
 * L'activité d'AlpIRC : la NativeActivity d'Android (Rust dessine tout et reçoit le toucher), plus ce qu'elle ne sait
 * pas faire (ADR 0006) :
 *
 * <ul>
 *   <li>l'affichage bord à bord (targetSdk 35) et les marges de l'écran passées à Rust ;
 *   <li>le champ de saisie natif en bas (clavier complet : autocorrection, suggestions, dictée, emoji, coller), avec
 *       les boutons Tab et Envoyer, dans une petite fenêtre au-dessus de celle de Rust (la surface de la
 *       NativeActivity appartient à Rust : les vues Java ne s'y dessinent pas) ;
 *   <li>le bouton Retour (Rust ferme ce qui est ouvert, sinon l'appli passe derrière, connexions gardées) ;
 *   <li>les liens {@code irc://}, la permission des notifications (Android 13+).
 * </ul>
 */
public class AlpircActivity extends NativeActivity {
    static volatile int insetLeft, insetTop, insetRight, insetBottom, imeBottom, panelBottom;
    /** Le code Rust a déjà tourné dans ce processus : une nouvelle activité ne peut que relancer le processus. */
    static boolean started;
    /** Demande (lien irc://) à transmettre au processus relancé. */
    static Intent forward;

    /** Sélecteur de documents en cours : code de la demande → jeton de Rust. */
    static final HashMap<Integer, Integer> picks = new HashMap<>();
    static int nextPickCode = 0x7000;

    LinearLayout panel;
    EditText edit;
    Button tabButton, sendButton;
    boolean panelWanted, panelAttached, resumed;
    int panelHeight;

    // Couleurs de Windows 98.
    static final int FACE = 0xFFC0C0C0, HILIGHT = 0xFFFFFFFF, SHADOW = 0xFF808080, DARK = 0xFF000000;

    @Override
    protected void onCreate(Bundle b) {
        Bridge.app = getApplicationContext();
        Bridge.activity = this;
        Bridge.clipboard = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
        boolean reused = started;
        started = true;
        forward = reused && b == null ? getIntent() : null;
        super.onCreate(b);
        // Bord à bord (imposé par Android 15 avec targetSdk 35) : Rust dessine tout l'écran et se tient à
        // l'intérieur des marges qu'on lui passe.
        Window w = getWindow();
        if (Build.VERSION.SDK_INT >= 30) {
            w.setDecorFitsSystemWindows(false);
        } else {
            w.getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        }
        if (Build.VERSION.SDK_INT >= 28) {
            WindowManager.LayoutParams lp = w.getAttributes();
            lp.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
            w.setAttributes(lp);
        }
        w.setStatusBarColor(Color.TRANSPARENT);
        w.setNavigationBarColor(Color.TRANSPARENT);
        w.getDecorView().setOnApplyWindowInsetsListener((v, ins) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                android.graphics.Insets bars = ins.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                android.graphics.Insets ime = ins.getInsets(WindowInsets.Type.ime());
                insetLeft = bars.left;
                insetTop = bars.top;
                insetRight = bars.right;
                insetBottom = bars.bottom;
                imeBottom = ime.bottom;
            } else {
                insetLeft = ins.getSystemWindowInsetLeft();
                insetTop = ins.getSystemWindowInsetTop();
                insetRight = ins.getSystemWindowInsetRight();
                insetBottom = ins.getSystemWindowInsetBottom();
                imeBottom = 0;
            }
            recheckPanel();
            Bridge.post("insets");
            return v.onApplyWindowInsets(ins);
        });
        buildPanel();
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[] {Manifest.permission.POST_NOTIFICATIONS}, 1);
        }
        // Processus déjà servi : Rust va le relancer, la demande suivra (voir Bridge.restart()).
        if (!reused) handle(getIntent());
    }

    @Override
    protected void onNewIntent(Intent i) {
        super.onNewIntent(i);
        setIntent(i);
        handle(i);
    }

    @Override
    protected void onResume() {
        super.onResume();
        resumed = true;
        Bridge.post("fg\t1");
        if (panelWanted) attachPanel();
    }

    @Override
    protected void onPause() {
        resumed = false;
        Bridge.post("fg\t0");
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        detachPanel();
        if (Bridge.activity == this) Bridge.activity = null;
        // Rust doit rendre la main pour que l'activité se termine : il ferme les connexions, enregistre, quitte.
        Bridge.post("activity\tdestroyed\t" + (isChangingConfigurations() ? "config" : "finish"));
        super.onDestroy();
    }

    /** Retour quand une vue Java a le focus (la touche ne passe pas par Rust) : Rust décide. */
    @Override
    public void onBackPressed() {
        Bridge.post("back");
    }

    /** Liens irc:// et ircs:// ouverts avec AlpIRC. */
    void handle(Intent i) {
        if (i == null || !Intent.ACTION_VIEW.equals(i.getAction())) return;
        Uri u = i.getData();
        if (u == null || u.getScheme() == null) return;
        String s = u.getScheme().toLowerCase(java.util.Locale.ROOT);
        if (s.equals("irc") || s.equals("ircs")) Bridge.post("url\t" + u.toString());
        // Une demande n'est traitée qu'une fois.
        setIntent(new Intent(Intent.ACTION_MAIN));
    }

    // --- Sélecteur de documents (envoi DCC, chargement d'un script…) ----------------------------------------

    /** Ouvre le sélecteur du système (ACTION_OPEN_DOCUMENT). */
    void pickFile(int token) {
        int code = nextPickCode++;
        if (nextPickCode > 0x7fff) nextPickCode = 0x7000;
        Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*");
        try {
            picks.put(code, token);
            startActivityForResult(i, code);
        } catch (Exception e) {
            picks.remove(code);
            Bridge.post("picked\t" + token);
        }
    }

    @Override
    protected void onActivityResult(int req, int res, Intent data) {
        Integer token = picks.remove(req);
        if (token == null) {
            super.onActivityResult(req, res, data);
            return;
        }
        Uri u = data == null ? null : data.getData();
        if (res != RESULT_OK || u == null) {
            Bridge.post("picked\t" + token);
            return;
        }
        // Rust lit des chemins : copie du document dans le cache, sous son nom (lecture hors du fil principal).
        new Thread(() -> {
            File f = copyToCache(u, token);
            Bridge.post("picked\t" + token + (f != null ? "\t" + f.getAbsolutePath() : ""));
        }).start();
    }

    File copyToCache(Uri u, int token) {
        String name = "document";
        try (Cursor c = getContentResolver().query(u, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst() && !c.isNull(0)) name = c.getString(0);
        } catch (Exception e) {
            // nom de repli
        }
        name = name.replace('/', '_').replace('\\', '_').replace('\t', ' ').replace('\n', ' ');
        if (name.isEmpty() || name.equals(".") || name.equals("..")) name = "document";
        File dir = new File(getCacheDir(), "picked/" + token);
        dir.mkdirs();
        File out = new File(dir, name);
        try (InputStream in = getContentResolver().openInputStream(u); OutputStream os = new FileOutputStream(out)) {
            if (in == null) return null;
            byte[] buf = new byte[64 * 1024];
            int n;
            while ((n = in.read(buf)) > 0) os.write(buf, 0, n);
            return out;
        } catch (Exception e) {
            Bridge.post("log\tcopie du document : " + e);
            return null;
        }
    }

    // --- Marges --------------------------------------------------------------------------------------------

    int dp(float v) {
        return Math.round(v * getResources().getDisplayMetrics().density);
    }

    /** Marges pour Rust : le bas couvre la barre de navigation, le clavier et le champ natif. */
    int[] insetsForRust() {
        int bottom = Math.max(insetBottom, imeBottom);
        if (panelAttached) bottom = Math.max(bottom + panelHeight, panelBottom);
        return new int[] {insetLeft, insetTop, insetRight, bottom};
    }

    /** Hauteur occupée par le champ natif depuis le bas de l'écran (sa position réelle, et sa hauteur). */
    void measurePanel() {
        int h = panelAttached ? panel.getHeight() : 0;
        int fromBottom = 0;
        if (panelAttached && panel.isAttachedToWindow() && h > 0) {
            int[] p = new int[2];
            panel.getLocationOnScreen(p);
            View decor = getWindow().getDecorView();
            int[] d = new int[2];
            decor.getLocationOnScreen(d);
            fromBottom = Math.max(0, d[1] + decor.getHeight() - p[1]);
        }
        if (h != panelHeight || fromBottom != panelBottom) {
            panelHeight = h;
            panelBottom = fromBottom;
            Bridge.post("insets");
        }
    }

    /** La fenêtre du champ natif bouge après coup (clavier) : on remesure un peu plus tard aussi. */
    void recheckPanel() {
        if (panel == null) return;
        panel.post(this::measurePanel);
        panel.postDelayed(this::measurePanel, 120);
        panel.postDelayed(this::measurePanel, 350);
    }

    // --- Champ de saisie natif -----------------------------------------------------------------------------

    /** Relief de Windows 98 : bord clair en haut à gauche, sombre en bas à droite (ou l'inverse : creux). */
    Drawable bevel(int fill, boolean sunken) {
        int a = sunken ? SHADOW : HILIGHT, b = sunken ? HILIGHT : DARK;
        int a2 = sunken ? DARK : FACE, b2 = sunken ? FACE : SHADOW;
        int px = Math.max(1, dp(1));
        LayerDrawable l = new LayerDrawable(new Drawable[] {new ColorDrawable(b), new ColorDrawable(a), new ColorDrawable(b2), new ColorDrawable(a2), new ColorDrawable(fill)});
        l.setLayerInset(0, 0, 0, 0, 0);
        l.setLayerInset(1, 0, 0, px, px);
        l.setLayerInset(2, px, px, 0, 0);
        l.setLayerInset(3, px, px, px, px);
        l.setLayerInset(4, 2 * px, 2 * px, 2 * px, 2 * px);
        return l;
    }

    Button button(String label) {
        Button bt = new Button(this);
        bt.setText(label);
        bt.setAllCaps(false);
        bt.setTextColor(DARK);
        bt.setTextSize(TypedValue.COMPLEX_UNIT_SP, 15);
        bt.setTypeface(Typeface.DEFAULT_BOLD);
        bt.setMinWidth(0);
        bt.setMinimumWidth(dp(48));
        bt.setMinHeight(0);
        bt.setMinimumHeight(dp(44));
        bt.setPadding(dp(10), 0, dp(10), 0);
        bt.setStateListAnimator(null);
        android.graphics.drawable.StateListDrawable s = new android.graphics.drawable.StateListDrawable();
        s.addState(new int[] {android.R.attr.state_pressed}, bevel(FACE, true));
        s.addState(new int[] {}, bevel(FACE, false));
        bt.setBackground(s);
        return bt;
    }

    void buildPanel() {
        panel = new LinearLayout(this) {
            @Override
            public boolean dispatchKeyEvent(KeyEvent e) {
                // Retour avec le champ natif au premier plan (le clavier, s'il est ouvert, l'a déjà pris).
                if (e.getKeyCode() == KeyEvent.KEYCODE_BACK) {
                    if (e.getAction() == KeyEvent.ACTION_UP && !e.isCanceled()) Bridge.post("back");
                    return true;
                }
                return super.dispatchKeyEvent(e);
            }
        };
        panel.setOrientation(LinearLayout.HORIZONTAL);
        panel.setGravity(Gravity.CENTER_VERTICAL);
        panel.setPadding(dp(3), dp(4), dp(3), dp(4));
        LayerDrawable bg = new LayerDrawable(new Drawable[] {new ColorDrawable(HILIGHT), new ColorDrawable(FACE)});
        bg.setLayerInset(1, 0, Math.max(1, dp(1)), 0, 0);
        panel.setBackground(bg);

        edit = new EditText(this);
        edit.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES | InputType.TYPE_TEXT_FLAG_AUTO_CORRECT);
        edit.setImeOptions(EditorInfo.IME_ACTION_SEND | EditorInfo.IME_FLAG_NO_FULLSCREEN | EditorInfo.IME_FLAG_NO_EXTRACT_UI);
        edit.setTextColor(DARK);
        edit.setHintTextColor(SHADOW);
        edit.setTextSize(TypedValue.COMPLEX_UNIT_SP, 16);
        edit.setTypeface(Typeface.MONOSPACE);
        edit.setBackground(bevel(HILIGHT, true));
        edit.setPadding(dp(8), dp(6), dp(8), dp(6));
        edit.setMinHeight(dp(44));
        edit.setOnEditorActionListener((v, action, ev) -> {
            boolean enter = ev != null && ev.getKeyCode() == KeyEvent.KEYCODE_ENTER;
            if (action == EditorInfo.IME_ACTION_SEND || action == EditorInfo.IME_ACTION_DONE || action == EditorInfo.IME_ACTION_GO || enter) {
                if (ev == null || ev.getAction() == KeyEvent.ACTION_DOWN) send();
                return true;
            }
            return false;
        });
        edit.setOnKeyListener((v, code, ev) -> {
            if (code == KeyEvent.KEYCODE_ENTER || code == KeyEvent.KEYCODE_NUMPAD_ENTER) {
                if (ev.getAction() == KeyEvent.ACTION_DOWN) send();
                return true;
            }
            if (code == KeyEvent.KEYCODE_TAB) {
                if (ev.getAction() == KeyEvent.ACTION_DOWN) Bridge.post("tab\t" + edit.getText());
                return true;
            }
            return false;
        });
        panel.addView(edit, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

        tabButton = button("Tab");
        tabButton.setOnClickListener(v -> Bridge.post("tab\t" + edit.getText()));
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, dp(44));
        lp.leftMargin = dp(3);
        panel.addView(tabButton, lp);

        sendButton = button("Send");
        sendButton.setOnClickListener(v -> send());
        LinearLayout.LayoutParams lp2 = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, dp(44));
        lp2.leftMargin = dp(3);
        panel.addView(sendButton, lp2);

        panel.addOnLayoutChangeListener((v, l, t, r, bo, ol, ot, or, ob) -> recheckPanel());
        panel.getViewTreeObserver().addOnGlobalLayoutListener(this::measurePanel);
        panel.setOnApplyWindowInsetsListener((v, ins) -> {
            recheckPanel();
            return v.onApplyWindowInsets(ins);
        });
    }

    void send() {
        String s = edit.getText().toString();
        if (s.trim().isEmpty()) return;
        Bridge.post("send\t" + s);
        edit.setText("");
    }

    void setInputText(String text) {
        edit.setText(text);
        edit.setSelection(edit.getText().length());
    }

    void setInputLabels(String send, String tab) {
        if (send != null && !send.isEmpty()) sendButton.setText(send);
        if (tab != null && !tab.isEmpty()) tabButton.setText(tab);
    }

    void setInputVisible(boolean visible, String hint) {
        if (hint != null) edit.setHint(hint);
        panelWanted = visible;
        if (visible) attachPanel();
        else detachPanel();
    }

    /**
     * Le champ natif vit dans une petite fenêtre (« panneau ») au-dessus de celle de la NativeActivity, collée en bas,
     * au-dessus de la barre de navigation et du clavier ; le toucher hors d'elle va à Rust.
     */
    void attachPanel() {
        if (panelAttached || !resumed) return;
        View decor = getWindow().getDecorView();
        if (decor.getWindowToken() == null) {
            decor.postDelayed(() -> {
                if (panelWanted) attachPanel();
            }, 100);
            return;
        }
        WindowManager.LayoutParams lp = new WindowManager.LayoutParams(
                WindowManager.LayoutParams.MATCH_PARENT,
                WindowManager.LayoutParams.WRAP_CONTENT,
                WindowManager.LayoutParams.TYPE_APPLICATION_PANEL,
                WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
                PixelFormat.OPAQUE);
        lp.gravity = Gravity.BOTTOM | Gravity.FILL_HORIZONTAL;
        lp.token = decor.getWindowToken();
        lp.softInputMode = WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE | WindowManager.LayoutParams.SOFT_INPUT_STATE_UNCHANGED;
        lp.setTitle("AlpIRC saisie");
        if (Build.VERSION.SDK_INT >= 30) {
            lp.setFitInsetsTypes(WindowInsets.Type.systemBars() | WindowInsets.Type.ime());
        }
        try {
            getWindowManager().addView(panel, lp);
            panelAttached = true;
        } catch (Exception e) {
            Bridge.post("log\tchamp de saisie : " + e);
        }
        recheckPanel();
    }

    void detachPanel() {
        if (!panelAttached) return;
        panelAttached = false;
        try {
            getWindowManager().removeViewImmediate(panel);
        } catch (Exception e) {
            Bridge.post("log\tchamp de saisie : " + e);
        }
        panelHeight = 0;
        panelBottom = 0;
        Bridge.post("insets");
    }

    /** Clavier du système pour les champs dessinés par Rust (le champ natif est alors retiré). */
    void showNativeKeyboard(boolean show, int attempt) {
        InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
        if (imm == null) return;
        View v = getCurrentFocus();
        if (v == null) v = getWindow().getDecorView();
        if (show) {
            if (panelAttached) return;
            boolean ok = imm.showSoftInput(v, 0);
            if (!ok && attempt < 3) {
                // Le focus revient à la fenêtre de Rust juste après le retrait du champ natif : on réessaie.
                getWindow().getDecorView().postDelayed(() -> showNativeKeyboard(true, attempt + 1), 150);
            }
        } else if (!panelAttached) {
            imm.hideSoftInputFromWindow(getWindow().getDecorView().getWindowToken(), 0);
        }
    }
}
