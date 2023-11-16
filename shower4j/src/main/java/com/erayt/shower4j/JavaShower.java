package com.erayt.shower4j;

import java.util.List;

public class JavaShower {

    private static final ByteConverter[] CONVERTERS = new ByteConverter[65536];

    public static boolean start() {
        return Shower.start();
    }

    public static void stop() {
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

    @SuppressWarnings("unchecked")
    public static <T> boolean newData(int id, T data) {
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
