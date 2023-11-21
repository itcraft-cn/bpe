package com.erayt.shower4j;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

public class JavaShower {
    private static final Logger LOGGER = LoggerFactory.getLogger(JavaShower.class);

    @SuppressWarnings("rawtypes")
    private static final ByteConverter[] CONVERTERS = new ByteConverter[65536];

    private static final long TIMEOUT = 5000L;

    private static final JavaShowerThread JAVA_SHOWER_THREAD = new JavaShowerThread();

    public static boolean start() {
        JAVA_SHOWER_THREAD.start();
        return Shower.start();
    }

    public static void stop() {
        JAVA_SHOWER_THREAD.stop(TIMEOUT);
        Shower.stop();
    }

    public static int defIncoming(String name, List<ColumnDefine> list) {
        int size = list.size();
        String[] names = new String[size];
        int[] types = new int[size];
        int[] lengths = new int[size];
        ColumnDefine.convert(list, names, types, lengths);
        return Shower.defIncoming(name, names, types, lengths);
    }

    public static int defStream(String name, List<ColumnDefine> list) {
        int size = list.size();
        String[] names = new String[size];
        int[] types = new int[size];
        int[] lengths = new int[size];
        ColumnDefine.convert(list, names, types, lengths);
        return Shower.defStream(name, names, types, lengths);
    }

    public static void regConvert(int id, ByteConverter<?> converter) {
        CONVERTERS[id] = converter;
    }

    public static <T> NewDataResult newDataSync(int id, T data) {
        try {
            return newDataAsync(id, data).get(TIMEOUT, TimeUnit.MILLISECONDS) ? NewDataResult.SUCCESSFUL : NewDataResult.FAILURE;
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
        JAVA_SHOWER_THREAD.fillQueue(new WrappedData<>(future, id, data));
        return future;
    }


    @SuppressWarnings("unchecked")
    static <T> boolean newData(int id, T data) {
        return Shower.newData(id, ((ByteConverter<T>) CONVERTERS[id]).convert(data));
    }

    public static int defMapper(String sql, ShowerCallback callback) {
        return Shower.defMapper(sql, callback);
    }

    public static int defMapperBindAggregate(String sql, int aggregateId) {
        return Shower.defMapperBindAggregate(sql, aggregateId);
    }

    public static int defAggregate(String sql, ShowerCallback callback) {
        return Shower.defAggregate(sql, callback);
    }
}
