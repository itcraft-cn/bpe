package com.erayt.shower4j;

import java.util.List;

/**
 * @author Helly Guo
 * <p>
 * Created on 11/16/23 3:42 PM
 */
public class ColumnDefine {

    private final String name;
    private final int type;
    private final int length;

    private ColumnDefine(String name, int type, int length) {
        this.name = name;
        this.type = type;
        this.length = length;
    }

    public static ColumnDefine createLong(String columnName) {
        return new ColumnDefine(columnName, 0, 0);
    }

    public static ColumnDefine createDouble(String columnName) {
        return new ColumnDefine(columnName, 1, 0);
    }

    public static ColumnDefine createString(String columnName, int size) {
        return new ColumnDefine(columnName, 2, size);
    }

    public static void convert(List<ColumnDefine> list, String[] names, int[] types, int[] lengths) {
        int size = list.size();
        if (names.length != size || types.length != size || lengths.length != size) {
            throw new IllegalArgumentException("list,names,types,lengths, the size is not match");
        }
        for (int i = 0; i < size; i++) {
            ColumnDefine define = list.get(i);
            names[i] = define.name;
            types[i] = define.type;
            lengths[i] = define.length;
        }
    }

}
