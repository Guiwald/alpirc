package fr.guiwald.alpirc;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import android.webkit.MimeTypeMap;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;

/**
 * Fournisseur de fichiers minimal (lecture seule) : donne aux autres applis, le temps d'un {@code ACTION_VIEW}, l'accès à
 * un fichier reçu par AlpIRC (DCC). Android interdit les adresses {@code file://} entre applis, et la plateforme n'a pas
 * de FileProvider sans bibliothèque. Seuls les fichiers des dossiers de l'appli sont servis ; le fournisseur n'est pas
 * exporté : l'accès passe par la permission accordée à l'adresse ({@code FLAG_GRANT_READ_URI_PERMISSION}).
 */
public class FilesProvider extends ContentProvider {
    static final String AUTHORITY = "fr.guiwald.alpirc.files";

    static Uri uriFor(File f) {
        return new Uri.Builder().scheme("content").authority(AUTHORITY).path(f.getAbsolutePath()).build();
    }

    static String mimeOf(String name) {
        int dot = name.lastIndexOf('.');
        String ext = dot >= 0 ? name.substring(dot + 1).toLowerCase(java.util.Locale.ROOT) : "";
        String t = MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext);
        return t != null ? t : "application/octet-stream";
    }

    static boolean under(File f, File root) throws IOException {
        if (root == null) return false;
        String r = root.getCanonicalPath() + File.separator;
        return f.getCanonicalPath().startsWith(r);
    }

    File fileFor(Uri u) throws FileNotFoundException {
        try {
            File f = new File(u.getPath()).getCanonicalFile();
            android.content.Context c = getContext();
            if (c != null && (under(f, c.getFilesDir()) || under(f, c.getCacheDir()) || under(f, c.getExternalFilesDir(null)))) return f;
        } catch (IOException e) {
            // refusé ci-dessous
        }
        throw new FileNotFoundException(String.valueOf(u));
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public String getType(Uri u) {
        return mimeOf(String.valueOf(u.getLastPathSegment()));
    }

    @Override
    public Cursor query(Uri u, String[] projection, String selection, String[] args, String sort) {
        File f;
        try {
            f = fileFor(u);
        } catch (FileNotFoundException e) {
            return null;
        }
        MatrixCursor c = new MatrixCursor(new String[] {OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE});
        c.addRow(new Object[] {f.getName(), f.length()});
        return c;
    }

    @Override
    public ParcelFileDescriptor openFile(Uri u, String mode) throws FileNotFoundException {
        if (!"r".equals(mode)) throw new FileNotFoundException("lecture seule");
        return ParcelFileDescriptor.open(fileFor(u), ParcelFileDescriptor.MODE_READ_ONLY);
    }

    @Override
    public Uri insert(Uri u, ContentValues v) {
        return null;
    }

    @Override
    public int delete(Uri u, String s, String[] a) {
        return 0;
    }

    @Override
    public int update(Uri u, ContentValues v, String s, String[] a) {
        return 0;
    }
}
