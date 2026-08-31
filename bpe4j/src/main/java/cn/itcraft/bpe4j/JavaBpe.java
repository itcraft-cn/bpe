package cn.itcraft.bpe4j;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
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
                return Bpe.start();
            } else {
                return true;
            }
        }
    }

    public static void stop() {
    }

    /** Callback delivery mode: synchronous, on the calling (ingest) thread. */
    public static final int DELIVERY_SYNC = 0;
    /** Callback delivery mode: asynchronous, on the engine's egress thread. */
    public static final int DELIVERY_ASYNC = 1;

    /**
     * Selects how callbacks are delivered. In {@link #DELIVERY_ASYNC} mode the
     * engine copies results into an egress ring and a dedicated thread invokes
     * the Java callbacks, so ingest is never blocked by callback execution.
     *
     * @param mode {@link #DELIVERY_SYNC} or {@link #DELIVERY_ASYNC}
     */
    public static void setDeliveryMode(int mode) {
        Bpe.setDeliveryMode(mode);
    }

    /**
     * Number of callback events dropped because the egress ring was full
     * (async mode only; drops are counted, never block ingest).
     */
    public static long droppedEvents() {
        return Bpe.droppedEvents();
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

    public static <T> NewDataResult newDataSync(int id, T data) {
        try {
            return newDataAsync(id, data).get(TIMEOUT, TimeUnit.MILLISECONDS)
                   ? NewDataResult.SUCCESSFUL : NewDataResult.FAILURE;
        } catch (InterruptedException e) {
            LOGGER.warn("interrupted: {}", e.getMessage());
            return NewDataResult.INTERRUPTED;
        } catch (ExecutionException e) {
            LOGGER.warn("execute failed: {}", e.getMessage(), e);
            return NewDataResult.EXECUTE_FAILED;
        } catch (TimeoutException e) {
            LOGGER.warn("timeout: {}", e.getMessage());
            return NewDataResult.TIMEOUT;
        }
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
