package com.erayt.shower4j;

public class Shower {
    static {
        // This actually loads the shared object that we'll be creating.
        // The actual location of the .so or .dll may differ based on your
        // platform.
        System.loadLibrary("shower4j");
    }

    public native static boolean start();

    public native static void stop();

    public native static boolean newData(int id, byte[] data);

    public native static boolean defAction(String sql);

    public native static boolean defActionWithCallback(String sql, ShowerCallback callback);
}
