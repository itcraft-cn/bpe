package com.erayt.shower4j;

class Shower {
    static {
        // This actually loads the shared object that we'll be creating.
        // The actual location of the .so or .dll may differ based on your
        // platform.
        System.loadLibrary("shower4j");
    }

    native static boolean start();

    native static void stop();

    native static int defIncoming(String name, String[] names, int[] types, int[] lengths);

    native static int defStream(String name, String[] names, int[] types, int[] lengths);

    native static boolean newData(int id, byte[] data);

    native static int defMapper(String sql, ShowerCallback callback);

    native static int defMapperBindAggregate(String sql, int aggregateId);

    native static int defAggregate(String sql, ShowerCallback callback);
}
