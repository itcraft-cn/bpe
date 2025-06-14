package cn.itcraft.bbpe4j;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicBoolean;

public class JavaBbpe {
    private static final Logger LOGGER = LoggerFactory.getLogger(JavaBbpe.class);

    @SuppressWarnings("rawtypes")
    private static final ByteConverter[] CONVERTERS = new ByteConverter[65536];

    private static final long TIMEOUT = 5000L;

    private static final AtomicBoolean INIT = new AtomicBoolean(false);
    private static final JavaBbpeThread JAVA_BBPE_THREAD = new JavaBbpeThread();

    public static boolean start() {
        synchronized (INIT) {
            if (INIT.compareAndSet(false, true)) {
                JAVA_BBPE_THREAD.start();
                Thread thread = new Thread(JavaBbpe::stop0, "bbpe-shutdown-hook");
                Runtime.getRuntime().addShutdownHook(thread);
                return Bbpe.start();
            } else {
                return true;
            }
        }
    }

    public static void stop() {
    }

    private static void stop0() {
        JAVA_BBPE_THREAD.stop(TIMEOUT);
        Bbpe.stop();
    }

    public static int defIncoming(String name, List<ColumnDefine> list) {
        int size = list.size();
        String[] names = new String[size];
        int[] types = new int[size];
        int[] lengths = new int[size];
        ColumnDefine.convert(list, names, types, lengths);
        return Bbpe.defIncoming(name, names, types, lengths);
    }

    public static int defStream(String name, List<ColumnDefine> list) {
        int size = list.size();
        String[] names = new String[size];
        int[] types = new int[size];
        int[] lengths = new int[size];
        ColumnDefine.convert(list, names, types, lengths);
        return Bbpe.defStream(name, names, types, lengths);
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
        JAVA_BBPE_THREAD.fillQueue(new WrappedData<>(future, id, data));
        return future;
    }


    @SuppressWarnings("unchecked")
    static <T> boolean newData(int id, T data) {
        return Bbpe.newData(id, ((ByteConverter<T>) CONVERTERS[id]).convert(data));
    }

    @SuppressWarnings("unchecked")
    static <T> T convert(int id, byte[] data, int offset) {
        return ((ByteConverter<T>) CONVERTERS[id]).convert(data, offset);
    }

    public static int defMapper(String sql, BbpeCallback callback) {
        return Bbpe.defMapper(sql, callback);
    }

    public static int defMapperBindAggregate(String sql, int aggregateId) {
        return Bbpe.defMapperBindAggregate(sql, aggregateId);
    }

    public static int defAggregate(String sql, BbpeCallback callback) {
        return Bbpe.defAggregate(sql, callback);
    }
}
