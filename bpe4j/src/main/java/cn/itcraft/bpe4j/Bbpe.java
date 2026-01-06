package cn.itcraft.bpe4j;

import cn.itcraft.nativeloader.NativeLoader;
import cn.itcraft.nativeloader.SimpleLibInfo;

final class Bpe {

    static {
        NativeLoader.load(new SimpleLibInfo("bpe4j"));
    }

    native static boolean start();

    native static void stop();

    native static int defIncoming(String name, String[] names, int[] types, int[] lengths);

    native static int defStream(String name, String[] names, int[] types, int[] lengths);

    native static boolean newData(int id, byte[] data);

    native static int defMapper(String sql, BpeCallback callback);

    native static int defMapperBindAggregate(String sql, int aggregateId);

    native static int defAggregate(String sql, BpeCallback callback);
}
