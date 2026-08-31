package cn.itcraft.bpe4j;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicBoolean;

public class JavaBpe {
    private static final Logger LOGGER = LoggerFactory.getLogger(JavaBpe.class);

    @SuppressWarnings("rawtypes")
    private static final ByteConverter[] CONVERTERS = new ByteConverter[65536];

    private static final long TIMEOUT = 5000L;

    private static final AtomicBoolean INIT = new AtomicBoolean(false);
    private static final JavaBpeThread JAVA_BPE_THREAD = new JavaBpeThread();

    public static boolean start() {
        synchronized (INIT) {
            if (INIT.compareAndSet(false, true)) {
                JAVA_BPE_THREAD.start();
                Thread thread = new Thread(JavaBpe::stop0, "bpe-shutdown-hook");
                Runtime.getRuntime().addShutdownHook(thread);
                boolean started = Bpe.start();
                // callbacks are delivered asynchronously on the egress thread
                Bpe.setDeliveryMode(DELIVERY_ASYNC);
                return started;
            } else {
                return true;
            }
        }
    }

    public static void stop() {
    }

    /** Asynchronous callback delivery (the only mode for Java). */
    public static final int DELIVERY_ASYNC = 1;

    /** Watchdog policy: discard queued backlog when a callback times out. */
    public static final int POLICY_DRAIN = 1;
    /** Watchdog policy: promote a new consumer thread; fall back to DRAIN when all hang. */
    public static final int POLICY_FAILOVER = 2;
    /** Watchdog policy: only raise alerts, never interfere. */
    public static final int POLICY_ALERT_ONLY = 3;

    /**
     * Configures the egress watchdog: what to do when a callback exceeds
     * {@code thresholdMs}. Default: {@link #POLICY_ALERT_ONLY} / 100 ms.
     */
    public static void setEgressPolicy(int policy, long thresholdMs) {
        Bpe.setEgressPolicy(policy, thresholdMs);
    }

    /**
     * Registers a listener invoked on callback timeouts. It receives a
     * 32-byte payload (four little-endian longs): {@code [elapsedMs][pending][dropped][policy]}.
     * Passing null restores the default (engine log alerts).
     */
    public static void setAlertListener(BpeCallback listener) {
        Bpe.setAlertListener(listener);
    }

    /** Events waiting in the egress channel. */
    public static long pendingEvents() {
        return Bpe.pendingEvents();
    }

    /** Events discarded by the DRAIN policy (the channel itself never drops). */
    public static long droppedEvents() {
        return Bpe.droppedEvents();
    }

    /** Total timeout alerts raised so far. */
    public static long alertCount() {
        return Bpe.alertCount();
    }

    private static void stop0() {
        JAVA_BPE_THREAD.stop(TIMEOUT);
        Bpe.stop();
    }

    public static int defIncoming(String name, List<ColumnDefine> list) {
        int size = list.size();
        String[] names = new String[size];
        int[] types = new int[size];
        int[] lengths = new int[size];
        ColumnDefine.convert(list, names, types, lengths);
        return Bpe.defIncoming(name, names, types, lengths);
    }

    public static int defStream(String name, List<ColumnDefine> list) {
        int size = list.size();
        String[] names = new String[size];
        int[] types = new int[size];
        int[] lengths = new int[size];
        ColumnDefine.convert(list, names, types, lengths);
        return Bpe.defStream(name, names, types, lengths);
    }

    public static void regConvert(int id, ByteConverter<?> converter) {
        CONVERTERS[id] = converter;
    }

    public static <T> CompletableFuture<Boolean> newDataAsync(int id, T data) {
        CompletableFuture<Boolean> future = new CompletableFuture<>();
        JAVA_BPE_THREAD.fillQueue(new WrappedData<>(future, id, data));
        return future;
    }


    @SuppressWarnings("unchecked")
    static <T> boolean newData(int id, T data) {
        return Bpe.newData(id, ((ByteConverter<T>) CONVERTERS[id]).convert(data));
    }

    @SuppressWarnings("unchecked")
    static <T> T convert(int id, byte[] data, int offset) {
        return ((ByteConverter<T>) CONVERTERS[id]).convert(data, offset);
    }

    public static int defMapper(String sql, BpeCallback callback) {
        return Bpe.defMapper(sql, callback);
    }

    public static int defMapperBindAggregate(String sql, int aggregateId) {
        return Bpe.defMapperBindAggregate(sql, aggregateId);
    }

    public static int defAggregate(String sql, BpeCallback callback) {
        return Bpe.defAggregate(sql, callback);
    }

    /** Fixed (tumbling) window type for {@link #defWindowAggregate}. */
    public static final int WINDOW_TUMBLING = 1;
    /** Sliding (hopping) window type for {@link #defWindowAggregate}. */
    public static final int WINDOW_SLIDING = 2;

    /**
     * Defines a time-window aggregate. The callback receives the aggregate result
     * bytes (same layout as {@link #defAggregate}) when a window ends.
     *
     * @param windowType {@link #WINDOW_TUMBLING} or {@link #WINDOW_SLIDING}
     * @param tsField    event-time column name (Long ms), or null for processing time
     * @param lagMs      out-of-order tolerance (watermark) / delay
     */
    public static int defWindowAggregate(String sql, int windowType, long periodMs, long lengthMs,
                                         long slideMs, String tsField, long lagMs,
                                         BpeCallback callback) {
        return Bpe.defWindowAggregate(sql, windowType, periodMs, lengthMs, slideMs, tsField, lagMs,
                                      callback);
    }

    /**
     * Defines a per-key time-window aggregate. The callback receives rows of
     * {@code [key(long)][field0]...[fieldN]} for every key in the window.
     */
    public static int defKeyedWindowAggregate(String sql, int windowType, long periodMs,
                                              long lengthMs, long slideMs, String tsField,
                                              String keyField, long lagMs, BpeCallback callback) {
        return Bpe.defKeyedWindowAggregate(sql, windowType, periodMs, lengthMs, slideMs, tsField,
                                           keyField, lagMs, callback);
    }

    /** Allocates a dimension table (used by {@code _dim_has/_dim_get} in SQL). */
    public static int defDimension() {
        return Bpe.defDimension();
    }

    /** Inserts/updates a key -> value entry in a dimension table. */
    public static void updateDimension(int id, long key, long value) {
        Bpe.updateDimension(id, key, value);
    }

    /** Removes a key from a dimension table. */
    public static void removeDimension(int id, long key) {
        Bpe.removeDimension(id, key);
    }
}
