package com.calsignlabs.metro_simulator;

public class Engine {
    public static native String hello(String input);

    static {
        System.loadLibrary("engine");
    }
}
