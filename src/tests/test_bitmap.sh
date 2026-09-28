#!/bin/bash
{
  # GETBIT bitmap 1 1
  # must return integer 0
  printf "*3\r\n\$6\r\nGETBIT\r\n\$6\r\nbitmap\r\n\$1\r\n3\r\n"

  # SETBIT bitmap 6 1
  printf "*4\r\n\$6\r\nSETBIT\r\n\$6\r\nbitmap\r\n\$1\r\n6\r\n\$1\r\n1\r\n"
  printf "*3\r\n\$6\r\nGETBIT\r\n\$6\r\nbitmap\r\n\$1\r\n6\r\n"

  # Test read String as bits
  printf "*3\r\n\$3\r\nSET\r\n\$5\r\nmykey\r\n\$1\r\nA\r\n"
  printf "*3\r\n\$6\r\nGETBIT\r\n\$5\r\nmykey\r\n\$1\r\n1\r\n"
  printf "*3\r\n\$6\r\nGETBIT\r\n\$5\r\nmykey\r\n\$1\r\n7\r\n"
  printf "*3\r\n\$6\r\nGETBIT\r\n\$5\r\nmykey\r\n\$1\r\n0\r\n"
  
  # Read bitmap as String, should receive @
  printf "*2\r\n\$3\r\nGET\r\n\$6\r\nbitmap\r\n"

  # Extend bitmap
  printf "*4\r\n\$6\r\nSETBIT\r\n\$6\r\nbitmap\r\n\$1\r\n8\r\n\$1\r\n1\r\n"
  printf "*2\r\n\$6\r\nSTRLEN\r\n\$6\r\nbitmap\r\n"

} | nc localhost 6379
