package com.erayt.bbpe4j;

import cn.itcraft.nativeloader.NativeLoader;
import cn.itcraft.nativeloader.SimpleLibInfo;

final class Bbpe {

    static {
        NativeLoader.load(new SimpleLibInfo("bbpe4j"));
    }

    native static boolean start();

    native static void stop();

    native static int defIncoming(String name, String[] names, int[] types, int[] lengths);

    native static int defStream(String name, String[] names, int[] types, int[] lengths);

    native static boolean newData(int id, byte[] data);

    native static int defMapper(String sql, BbpeCallback callback);

    native static int defMapperBindAggregate(String sql, int aggregateId);

    native static int defAggregate(String sql, BbpeCallback callback);
}
