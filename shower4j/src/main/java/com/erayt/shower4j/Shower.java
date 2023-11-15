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

    public native static int defIncoming(String name, String[] names, int[] types, int[] lengths);

    public native static int defStream(String name, String[] names, int[] types, int[] lengths);

    public native static boolean newData(int id, byte[] data);

    public native static int defMapper(String sql, ShowerCallback callback);

    public native static int defMapperBindAggregate(String sql, int aggregateId);

    public native static int defAggregate(String sql, ShowerCallback callback);
}
