package cn.itcraft.bpe4j;

import cn.itcraft.nativeloader.NativeLoader;
import cn.itcraft.nativeloader.SimpleLibInfo;

final class Bpe {

    static {
        NativeLoader.load(new SimpleLibInfo("bpe4j"));
    }

    native static boolean start();

    native static void stop();

    native static void setDeliveryMode(int mode);

    native static long droppedEvents();

    native static long pendingEvents();

    native static long alertCount();

    native static void setEgressPolicy(int policy, long thresholdMs);

    native static void setAlertListener(BpeCallback listener);

    native static int defIncoming(String name, String[] names, int[] types, int[] lengths);

    native static int defStream(String name, String[] names, int[] types, int[] lengths);

    native static boolean newData(int id, byte[] data);

    native static int defMapper(String sql, BpeCallback callback);

    native static int defMapperBindAggregate(String sql, int aggregateId);

    native static int defAggregate(String sql, BpeCallback callback);

    native static int defWindowAggregate(String sql, int windowType, long periodMs, long lengthMs,
                                         long slideMs, String tsField, long lagMs,
                                         BpeCallback callback);

    native static int defKeyedWindowAggregate(String sql, int windowType, long periodMs,
                                              long lengthMs, long slideMs, String tsField,
                                              String keyField, long lagMs, BpeCallback callback);

    native static int defDimension();

    native static void updateDimension(int id, long key, long value);

    native static void removeDimension(int id, long key);
}
