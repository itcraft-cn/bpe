package com.erayt.shower4j;

class Shower {
    static {
        NativeLoader.load();
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
