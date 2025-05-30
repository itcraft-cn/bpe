package com.erayt.bambootube4j;

import cn.itcraft.nativeloader.NativeLoader;
import cn.itcraft.nativeloader.SimpleLibInfo;

final class BambooTube {

    static {
        NativeLoader.load(new SimpleLibInfo("bambootube4j"));
    }

    native static boolean start();

    native static void stop();

    native static int defIncoming(String name, String[] names, int[] types, int[] lengths);

    native static int defStream(String name, String[] names, int[] types, int[] lengths);

    native static boolean newData(int id, byte[] data);

    native static int defMapper(String sql, BambooTubeCallback callback);

    native static int defMapperBindAggregate(String sql, int aggregateId);

    native static int defAggregate(String sql, BambooTubeCallback callback);
}
