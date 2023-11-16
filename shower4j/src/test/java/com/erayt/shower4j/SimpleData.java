package com.erayt.shower4j;

import java.util.StringJoiner;

/**
 * @author Helly Guo
 * <p>
 * Created on 11/16/23 4:46 PM
 */
class SimpleData {
    private int val1;
    private long val2;
    private double val3;
    private String val4;

    public SimpleData() {
    }

    public SimpleData(int val1, long val2, double val3, String val4) {
        this.val1 = val1;
        this.val2 = val2;
        this.val3 = val3;
        this.val4 = val4;
    }

    public int getVal1() {
        return val1;
    }

    public void setVal1(int val1) {
        this.val1 = val1;
    }

    public long getVal2() {
        return val2;
    }

    public void setVal2(long val2) {
        this.val2 = val2;
    }

    public double getVal3() {
        return val3;
    }

    public void setVal3(double val3) {
        this.val3 = val3;
    }

    public String getVal4() {
        return val4;
    }

    public void setVal4(String val4) {
        this.val4 = val4;
    }

    @Override
    public String toString() {
        return new StringJoiner(", ", SimpleData.class.getSimpleName() + "[", "]")
                .add("val1=" + val1)
                .add("val2=" + val2)
                .add("val3=" + val3)
                .add("val4='" + val4 + "'")
                .toString();
    }
}
