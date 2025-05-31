package com.erayt.bambootube4j;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

public class JavaBambooTube {
    private static final Logger LOGGER = LoggerFactory.getLogger(JavaBambooTube.class);

    @SuppressWarnings("rawtypes")
    private static final ByteConverter[] CONVERTERS = new ByteConverter[65536];

    private static final long TIMEOUT = 5000L;

    private static final JavaBambooTubeThread JAVA_BAMBOOTUBE_THREAD = new JavaBambooTubeThread();

    public static boolean start() {
        JAVA_BAMBOOTUBE_THREAD.start();
        return BambooTube.start();
    }

    public static void stop() {
        JAVA_BAMBOOTUBE_THREAD.stop(TIMEOUT);
        BambooTube.stop();
    }

    public static int defIncoming(String name, List<ColumnDefine> list) {
        int size = list.size();
        String[] names = new String[size];
        int[] types = new int[size];
        int[] lengths = new int[size];
        ColumnDefine.convert(list, names, types, lengths);
        return BambooTube.defIncoming(name, names, types, lengths);
    }

    public static int defStream(String name, List<ColumnDefine> list) {
        int size = list.size();
        String[] names = new String[size];
        int[] types = new int[size];
        int[] lengths = new int[size];
        ColumnDefine.convert(list, names, types, lengths);
        return BambooTube.defStream(name, names, types, lengths);
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
        JAVA_BAMBOOTUBE_THREAD.fillQueue(new WrappedData<>(future, id, data));
        return future;
    }


    @SuppressWarnings("unchecked")
    static <T> boolean newData(int id, T data) {
        return BambooTube.newData(id, ((ByteConverter<T>) CONVERTERS[id]).convert(data));
    }

    @SuppressWarnings("unchecked")
    static <T> T convert(int id, byte[] data, int offset) {
        return ((ByteConverter<T>) CONVERTERS[id]).convert(data, offset);
    }

    public static int defMapper(String sql, BambooTubeCallback callback) {
        return BambooTube.defMapper(sql, callback);
    }

    public static int defMapperBindAggregate(String sql, int aggregateId) {
        return BambooTube.defMapperBindAggregate(sql, aggregateId);
    }

    public static int defAggregate(String sql, BambooTubeCallback callback) {
        return BambooTube.defAggregate(sql, callback);
    }
}
