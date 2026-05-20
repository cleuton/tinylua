-- TinyLua Blink
-- Acende o LED por 1 segundo, apaga por 1 segundo, repete indefinidamente.
-- Equivalente ao exemplo Blink padrão do Arduino IDE.
-- Pino 13 = LED_BUILTIN no Arduino UNO

local LED_PIN = 13

pinMode(LED_PIN, "OUTPUT")

while true do
    digitalWrite(LED_PIN, true)   -- HIGH
    delay(1000)
    digitalWrite(LED_PIN, false)  -- LOW
    delay(1000)
end
