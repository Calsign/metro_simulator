package com.calsignlabs.metro_simulator;

import android.app.Activity;
import android.os.Bundle;
import android.widget.TextView;

import com.calsignlabs.metro_simulator.Engine;

public class MainActivity extends Activity {
    @Override
    public void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        TextView helloWorld = findViewById(R.id.helloWorld);
        helloWorld.setText(Engine.hello("engine"));
    }
}
